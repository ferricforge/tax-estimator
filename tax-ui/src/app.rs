#![allow(unused)]
use std::fmt;

use rust_decimal::Decimal;
use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
use tracing::debug;

use tax_core::db::{DbConfig, RepositoryRegistry, TaxRepository};
use tax_core::models::{FilingStatus, StandardDeduction, TaxBracket, TaxYearConfig};
use tax_db_sqlite::SqliteRepositoryFactory;

use crate::utils::{currency, percent};

// ─── public data types ───────────────────────────────────────────────────────

/// Reference data for one filing status: the status row itself, its
/// standard deduction for the year, and every bracket that applies.
#[derive(Debug, Clone)]
pub struct FilingStatusData {
    pub filing_status: FilingStatus,
    pub standard_deduction: StandardDeduction,
    pub tax_brackets: Vec<TaxBracket>,
}

/// Everything the calculator needs to know about a single tax year,
/// gathered into one place.  Built by [`load_tax_year_data`].
#[derive(Debug, Clone)]
pub struct TaxYearData {
    pub config: TaxYearConfig,
    /// One entry per filing status, each carrying its deduction and brackets.
    pub statuses: Vec<FilingStatusData>,
}

// ─── registry ────────────────────────────────────────────────────────────────

/// Register every known backend with a fresh [`RepositoryRegistry`].
/// Adding a new backend later is one line here.
pub fn build_registry() -> RepositoryRegistry {
    let mut registry = RepositoryRegistry::new();
    registry.register(Box::new(SqliteRepositoryFactory));
    registry
}

// ─── loading ─────────────────────────────────────────────────────────────────

/// Pull every piece of reference data the calculator needs for `year`:
/// the year config, every filing status, and its standard deduction +
/// tax brackets.
///
/// Logs each stage at `debug` level so the caller can trace progress
/// without cluttering normal output.
pub async fn load_tax_year_data(
    repo: &dyn TaxRepository,
    year: i32,
) -> anyhow::Result<TaxYearData> {
    debug!("loading tax-year config for {year}");
    let config = repo.get_tax_year_config(year).await?;

    debug!("loading filing statuses");
    let statuses = repo.list_filing_statuses().await?;

    let mut status_data = Vec::with_capacity(statuses.len());
    for status in statuses {
        debug!(
            "loading deduction + brackets for {}",
            status.status_code.as_str()
        );

        let deduction = repo.get_standard_deduction(year, status.id).await?;
        let brackets = repo.get_tax_brackets(year, status.id).await?;

        status_data.push(FilingStatusData {
            filing_status: status,
            standard_deduction: deduction,
            tax_brackets: brackets,
        });
    }

    Ok(TaxYearData {
        config,
        statuses: status_data,
    })
}

// ─── formatting helpers ──────────────────────────────────────────────────────
// Private; only used by the Display impls below.  Both force exactly two
// decimal places so the output is stable regardless of how the Decimal
// was originally constructed.

// ─── Display ─────────────────────────────────────────────────────────────────
impl fmt::Display for FilingStatusData {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(
            f,
            "{} ({})",
            self.filing_status.status_name,
            self.filing_status.status_code.as_str()
        )?;
        writeln!(
            f,
            "  Standard deduction: {}",
            currency(&self.standard_deduction.amount)
        )?;
        writeln!(f, "  Tax brackets:")?;

        for b in &self.tax_brackets {
            // Capped brackets:   "$0.00 to $11600.00"
            // Open-ended (top):  "$609350.00 and above"
            let range = match &b.max_income {
                Some(max) => format!("{} to {}", currency(&b.min_income), currency(max)),
                None => format!("{} and above", currency(&b.min_income)),
            };
            writeln!(
                f,
                "    {:30} {:>6}  base {}",
                range,
                percent(&b.tax_rate),
                currency(&b.base_tax),
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for TaxYearData {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        // ── TaxYearConfig fields (foreign type, inlined) ──────────────
        let c = &self.config;
        writeln!(f, "{}", c)?;
        // ── one block per filing status, each preceded by a blank line ─
        for status in &self.statuses {
            writeln!(f)?;
            write!(f, "{}", status)?;
        }
        Ok(())
    }
}

pub async fn se_tax_estimate() {
    let db_config = DbConfig {
        backend: "sqlite".to_string(),
        connection_string: ":memory:".to_string(),
    };
    let registry = build_registry();
    let repo = registry
        .create(&db_config)
        .await
        .expect("repository creation should succeed");
}

fn run_se_worksheet(
    config: &TaxYearConfig,
    se_income: Decimal,
    crp_payments: Decimal,
    wages: Decimal,
) -> tax_core::calculations::SeWorksheetResult {
    let se_config = SeWorksheetConfig::from_tax_year_config(config);
    let worksheet = SeWorksheet::new(se_config);
    worksheet
        .calculate(se_income, crp_payments, wages)
        .expect("SE worksheet calculation should succeed")
}

// ─── tests ───────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use tax_core::models::{
        FilingStatus, FilingStatusCode, StandardDeduction, TaxBracket, TaxYearConfig,
    };

    use super::{FilingStatusData, TaxYearData};

    // ── test-data builders ──────────────────────────────────────────────
    // Each builder produces the minimal, realistic shape needed by the
    // tests below.  Values are chosen so that their formatted forms are
    // unique strings — "$15000.00" never collides with "$30000.00", etc.

    fn sample_config() -> TaxYearConfig {
        TaxYearConfig {
            tax_year: 2025,
            ss_wage_max: dec!(176_100),
            ss_tax_rate: dec!(0.062),
            medicare_tax_rate: dec!(0.0145),
            se_tax_deduct_pcnt: dec!(0.5),
            se_deduction_factor: dec!(0.9235),
            req_pmnt_threshold: dec!(1_000),
            min_se_threshold: dec!(400),
        }
    }

    /// Single / two brackets: one capped, one open-ended.  Exercises both
    /// branch paths of the range formatter in a single fixture.
    fn single_status_data() -> FilingStatusData {
        FilingStatusData {
            filing_status: FilingStatus {
                id: 1,
                status_code: FilingStatusCode::Single,
                status_name: "Single".to_string(),
            },
            standard_deduction: StandardDeduction {
                tax_year: 2025,
                filing_status_id: 1,
                amount: dec!(15_000),
            },
            tax_brackets: vec![
                TaxBracket {
                    tax_year: 2025,
                    filing_status_id: 1,
                    min_income: dec!(0),
                    max_income: Some(dec!(11_600)),
                    tax_rate: dec!(0.10),
                    base_tax: dec!(0),
                },
                TaxBracket {
                    tax_year: 2025,
                    filing_status_id: 1,
                    min_income: dec!(11_600),
                    max_income: None, // open-ended
                    tax_rate: dec!(0.12),
                    base_tax: dec!(1_160),
                },
            ],
        }
    }

    /// MFJ / one bracket.  Deduction ($30 000) and status code (MFJ) are
    /// distinct from Single so the multi-status test can assert both appear.
    fn mfj_status_data() -> FilingStatusData {
        FilingStatusData {
            filing_status: FilingStatus {
                id: 2,
                status_code: FilingStatusCode::MarriedFilingJointly,
                status_name: "Married Filing Jointly".to_string(),
            },
            standard_deduction: StandardDeduction {
                tax_year: 2025,
                filing_status_id: 2,
                amount: dec!(30_000),
            },
            tax_brackets: vec![TaxBracket {
                tax_year: 2025,
                filing_status_id: 2,
                min_income: dec!(0),
                max_income: Some(dec!(23_200)),
                tax_rate: dec!(0.10),
                base_tax: dec!(0),
            }],
        }
    }

    // ── FilingStatusData Display ────────────────────────────────────────

    #[test]
    fn status_display_contains_name_code_deduction_and_base_tax() {
        let out = format!("{}", single_status_data());

        assert!(out.contains("Single (S)"), "name + code");
        assert!(out.contains("$15000.00"), "standard deduction");
        assert!(out.contains("10.00%"), "first bracket rate");
        assert!(out.contains("base $0.00"), "first bracket base tax");
    }

    /// The two bracket variants ("to" and "and above") live in the same
    /// fixture; one test, two assertions, zero duplication.
    #[test]
    fn bracket_range_capped_uses_to_open_uses_and_above() {
        let out = format!("{}", single_status_data());

        assert!(
            out.contains("$0.00 to $11600.00"),
            "capped bracket should use 'to'"
        );
        assert!(
            out.contains("$11600.00 and above"),
            "open-ended bracket should use 'and above'"
        );
    }

    // ── TaxYearData Display ─────────────────────────────────────────────

    /// Every field of TaxYearConfig must appear in the output.  The
    /// se_deduction_factor is a plain multiplier — it must NOT be run
    /// through percent().
    #[test]
    fn full_output_contains_every_config_field() {
        let data = TaxYearData {
            config: sample_config(),
            statuses: vec![single_status_data()],
        };
        let out = format!("{}", data);

        assert!(out.contains("Tax Year Configuration (2025)"));
        assert!(out.contains("$176100.00"), "ss_wage_max");
        assert!(out.contains("6.20%"), "ss_tax_rate");
        assert!(out.contains("1.45%"), "medicare_tax_rate");
        assert!(out.contains("50.00%"), "se_tax_deductible_percentage");
        assert!(out.contains("0.9235"), "se_deduction_factor — plain, not %");
        assert!(out.contains("$1000.00"), "required_payment_threshold");
        assert!(out.contains("$400.00"), "min_se_threshold");
    }

    /// Two statuses must both appear.  A blank line (`\n\n`) separates
    /// each status block from the one before it.
    #[test]
    fn multiple_statuses_all_present_with_blank_line_separators() {
        let data = TaxYearData {
            config: sample_config(),
            statuses: vec![single_status_data(), mfj_status_data()],
        };
        let out = format!("{}", data);

        assert!(out.contains("Single (S)"), "first status present");
        assert!(
            out.contains("Married Filing Jointly (MFJ)"),
            "second status present"
        );
        assert!(out.contains("$30000.00"), "MFJ deduction distinguishes it");
        assert!(out.contains("\n\n"), "blank-line separator between blocks");
    }
}
