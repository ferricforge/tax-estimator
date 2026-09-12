//! Reference data for a single tax year, shaped for the calculator UI.
//!
//! [`TaxYearData`] bundles the year's configuration with, for each filing
//! status, its standard deduction and bracket table. It is loaded once per
//! year via [`TaxYearData::load`] and then handed to the worksheets.

use std::fmt;

use anyhow::{Context, Result};
use tax_core::db::TaxRepository;
use tax_core::models::{
    FilingStatus, FilingStatusCode, StandardDeduction, TaxBracket, TaxYearConfig,
};
use tracing::debug;

use crate::utils::{currency, percent};

/// Reference data for one filing status: the status row itself, its
/// standard deduction for the year, and every bracket that applies.
#[derive(Debug, Clone)]
pub struct FilingStatusData {
    pub filing_status: FilingStatus,
    pub standard_deduction: StandardDeduction,
    pub tax_brackets: Vec<TaxBracket>,
}

impl From<(FilingStatus, StandardDeduction, Vec<TaxBracket>)> for FilingStatusData {
    fn from(
        (filing_status, standard_deduction, tax_brackets): (
            FilingStatus,
            StandardDeduction,
            Vec<TaxBracket>,
        )
    ) -> Self {
        Self {
            filing_status,
            standard_deduction,
            tax_brackets,
        }
    }
}

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

/// Everything the calculator needs to know about a single tax year.
#[derive(Debug, Clone)]
pub struct TaxYearData {
    pub config: TaxYearConfig,
    /// One entry per filing status, each carrying its deduction and brackets.
    pub statuses: Vec<FilingStatusData>,
}

impl TaxYearData {
    /// Pull every piece of reference data the calculator needs for `year`:
    /// the year config, every filing status, and its standard deduction +
    /// tax brackets.
    pub async fn load(
        repo: &dyn TaxRepository,
        year: i32,
    ) -> Result<Self> {
        debug!("loading tax-year data for {year}");

        let (config, status_rows) = tokio::try_join!(
            repo.get_tax_year_config(year),
            repo.get_filing_status_data(year),
        )
        .with_context(|| format!("failed to load reference data for tax year {year}"))?;

        let statuses = status_rows
            .into_iter()
            .map(FilingStatusData::from)
            .collect();

        Ok(Self { config, statuses })
    }

    /// The reference data for a single filing status, if the year has it.
    pub fn status_for(
        &self,
        code: FilingStatusCode,
    ) -> Option<&FilingStatusData> {
        self.statuses
            .iter()
            .find(|s| s.filing_status.status_code == code)
    }
}

impl fmt::Display for TaxYearData {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "{}", self.config)?;
        // One block per filing status, each preceded by a blank line.
        for status in &self.statuses {
            writeln!(f)?;
            write!(f, "{status}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::models::{
        FilingStatus, FilingStatusCode, StandardDeduction, TaxBracket, TaxYearConfig,
    };

    use super::*;

    // ── test-data builders ──────────────────────────────────────────────
    // Values are chosen so that their formatted forms are unique strings —
    // "$15000.00" never collides with "$30000.00", etc.

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

    /// Single / two brackets: one capped, one open-ended.
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
                    max_income: None,
                    tax_rate: dec!(0.12),
                    base_tax: dec!(1_160),
                },
            ],
        }
    }

    /// MFJ / one bracket.
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

    fn sample_year_data() -> TaxYearData {
        TaxYearData {
            config: sample_config(),
            statuses: vec![single_status_data(), mfj_status_data()],
        }
    }

    #[test]
    fn from_tuple_maps_fields_in_order() {
        let src = single_status_data();
        let tuple = (
            src.filing_status.clone(),
            src.standard_deduction.clone(),
            src.tax_brackets.clone(),
        );

        let data = FilingStatusData::from(tuple);

        assert_eq!(data.filing_status.id, 1);
        assert_eq!(data.standard_deduction.amount, dec!(15_000));
        assert_eq!(data.tax_brackets.len(), 2);
    }

    #[test]
    fn status_for_finds_matching_status() {
        let data = sample_year_data();

        let mfj = data
            .status_for(FilingStatusCode::MarriedFilingJointly)
            .expect("MFJ should be present");

        assert_eq!(mfj.filing_status.id, 2);
        assert_eq!(mfj.standard_deduction.amount, dec!(30_000));
    }

    #[test]
    fn status_for_returns_none_for_missing_status() {
        let data = TaxYearData {
            config: sample_config(),
            statuses: vec![single_status_data()],
        };

        assert!(
            data.status_for(FilingStatusCode::MarriedFilingJointly)
                .is_none()
        );
    }

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

    #[test]
    fn multiple_statuses_all_present_with_blank_line_separators() {
        let out = format!("{}", sample_year_data());

        assert!(out.contains("Single (S)"), "first status present");
        assert!(
            out.contains("Married Filing Jointly (MFJ)"),
            "second status present"
        );
        assert!(out.contains("$30000.00"), "MFJ deduction distinguishes it");
        assert!(out.contains("\n\n"), "blank-line separator between blocks");
    }
}
