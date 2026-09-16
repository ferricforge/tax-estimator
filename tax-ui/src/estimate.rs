//! Glue between the UI and `tax-core`: run the worksheets for a
//! [`TaxEstimateInput`] and persist the result.
//!
//! Nothing in here knows about gpui. Widgets collect input, call into this
//! module, and render whatever comes back.

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use tax_core::calculations::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetContext, EstimatedTaxWorksheetResult, SeWorksheet,
    SeWorksheetConfig, SeWorksheetResult,
};
use tax_core::db::TaxRepository;
use tax_core::models::TaxYearConfig;
use tax_core::{FilingStatusCode, TaxEstimate, TaxEstimateComputed, TaxEstimateInput};
use tracing::debug;

use crate::models::FilingStatusData;
use crate::state::ActiveTaxYear;

/// Why an estimate could not be calculated for the input the user entered.
#[derive(Debug, thiserror::Error)]
pub enum EstimateError {
    /// No tax-year configuration is loaded for `year`.
    #[error("no tax year configuration is loaded for {year}")]
    TaxYearNotLoaded { year: i32 },
    /// The loaded year has no bracket data for `status`.
    #[error("no tax bracket data for filing status {status:?} in tax year {year}")]
    MissingFilingStatus { year: i32, status: FilingStatusCode },
    /// One of the worksheets rejected the input.
    #[error(transparent)]
    Worksheet(#[from] anyhow::Error),
}

/// Runs the Estimated Tax Worksheet for `input` using whatever tax-year data
/// is currently loaded.
///
/// Fails with [`EstimateError::TaxYearNotLoaded`] unless `active` holds the
/// configuration for `input.tax_year`, and with
/// [`EstimateError::MissingFilingStatus`] when that configuration has no
/// brackets for `input.filing_status`.
pub fn calculate_estimate(
    active: &ActiveTaxYear,
    input: &TaxEstimateInput,
    se_tax: Decimal,
) -> Result<EstimatedTaxWorksheetResult, EstimateError> {
    let tax_year_data = active
        .data_for(input.tax_year)
        .ok_or(EstimateError::TaxYearNotLoaded {
            year: input.tax_year,
        })?;

    let status = tax_year_data.status_for(input.filing_status).ok_or(
        EstimateError::MissingFilingStatus {
            year: input.tax_year,
            status: input.filing_status,
        },
    )?;

    Ok(estimated_tax(status, &tax_year_data.config, input, se_tax)?)
}

/// Runs the Self-Employment Tax Worksheet for the given income figures.
pub fn se_tax_estimate(
    config: &TaxYearConfig,
    se_income: Decimal,
    crp_payments: Decimal,
    wages: Decimal,
) -> Result<SeWorksheetResult> {
    let worksheet = SeWorksheet::new(SeWorksheetConfig::from_tax_year_config(config));
    let result = worksheet
        .calculate(se_income, crp_payments, wages)
        .with_context(|| {
            format!(
                "SE worksheet calculation failed \
                 (se_income={se_income}, crp_payments={crp_payments}, wages={wages})"
            )
        })?;

    debug!("SE worksheet result:\n{result}");
    Ok(result)
}

/// Runs the Estimated Tax Worksheet for `input` using the brackets for
/// `status` and the year's payment threshold from `config`.
///
/// `se_tax` is the self-employment tax from [`se_tax_estimate`] (or zero
/// when the taxpayer has no SE income).
pub fn estimated_tax(
    status: &FilingStatusData,
    config: &TaxYearConfig,
    input: &TaxEstimateInput,
    se_tax: Decimal,
) -> Result<EstimatedTaxWorksheetResult> {
    let worksheet_input = input.to_estimated_tax_worksheet_input(&EstimatedTaxWorksheetContext {
        self_employment_tax: se_tax,
        refundable_credits: Decimal::ZERO,
        is_farmer_or_fisher: false,
        required_payment_threshold: config.req_pmnt_threshold,
    });

    let result = EstimatedTaxWorksheet::new(&status.tax_brackets)
        .calculate(&worksheet_input)
        .with_context(|| {
            format!(
                "estimated tax worksheet calculation failed for {} / {}",
                input.tax_year,
                status.filing_status.status_code.as_str()
            )
        })?;

    debug!("Estimated tax worksheet result:\n{result}");
    Ok(result)
}

/// Builds the persisted summary of a calculation from the two worksheet
/// outputs. Centralises the field mapping so it is written exactly once.
pub fn computed_values(
    se_tax: Decimal,
    estimate: &EstimatedTaxWorksheetResult,
) -> TaxEstimateComputed {
    TaxEstimateComputed {
        se_tax,
        total_tax: estimate.total_estimated_tax,
        required_payment: estimate.required_annual_payment,
    }
}

/// Persists a new estimate: creates the record from `input`, then stores
/// the `computed` summary against it. Returns the stored estimate so the
/// caller has its id.
pub async fn save_tax_estimate(
    repo: &dyn TaxRepository,
    input: &TaxEstimateInput,
    computed: TaxEstimateComputed,
) -> Result<TaxEstimate> {
    let mut estimate = repo
        .create_estimate(input.clone())
        .await
        .context("failed to create tax estimate")?;

    estimate.computed = Some(computed);

    repo.update_estimate(&estimate)
        .await
        .context("failed to store computed values for tax estimate")?;

    Ok(estimate)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::FilingStatusCode;
    use tax_core::models::{FilingStatus, StandardDeduction, TaxBracket};

    use super::*;
    use crate::models::TaxYearData;

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

    /// Same three brackets as the `tax-core` doc example, so the expected
    /// figures below are already verified there.
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
                    max_income: Some(dec!(11_925)),
                    tax_rate: dec!(0.10),
                    base_tax: dec!(0),
                },
                TaxBracket {
                    tax_year: 2025,
                    filing_status_id: 1,
                    min_income: dec!(11_925),
                    max_income: Some(dec!(48_475)),
                    tax_rate: dec!(0.12),
                    base_tax: dec!(1_192.50),
                },
                TaxBracket {
                    tax_year: 2025,
                    filing_status_id: 1,
                    min_income: dec!(48_475),
                    max_income: Some(dec!(103_350)),
                    tax_rate: dec!(0.22),
                    base_tax: dec!(5_578.50),
                },
            ],
        }
    }

    fn wage_only_input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: Some(dec!(100_000.00)),
            expected_agi: dec!(100_000.00),
            expected_deduction: dec!(15_000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: Some(dec!(12_000.00)),
        }
    }

    fn active_tax_year(
        year: Option<i32>,
        statuses: Vec<FilingStatusData>,
    ) -> ActiveTaxYear {
        let tax_year_data = TaxYearData {
            config: sample_config(),
            statuses,
        };

        match year {
            Some(year) => ActiveTaxYear::loaded(year, tax_year_data),
            None => ActiveTaxYear::default(),
        }
    }

    #[test]
    fn se_tax_estimate_succeeds_for_typical_input() {
        let result = se_tax_estimate(
            &sample_config(),
            dec!(100_000.00),
            Decimal::ZERO,
            dec!(50_000.00),
        );

        // Exact figures are covered by tax-core's own SE worksheet tests;
        // here we only care that the glue wires config + inputs through.
        let result = result.expect("SE worksheet should succeed");
        assert!(result.self_employment_tax > Decimal::ZERO);
    }

    #[test]
    fn estimated_tax_matches_tax_core_doc_example() {
        let status = single_status_data();
        let config = sample_config();

        let result = estimated_tax(&status, &config, &wage_only_input(), Decimal::ZERO)
            .expect("estimated tax worksheet should succeed");

        // Taxable income: 100000 - 15000 = 85000
        // Tax: 5578.50 + (85000 - 48475) * 0.22 = 13614
        assert_eq!(result.taxable_income, dec!(85_000.00));
        assert_eq!(result.total_estimated_tax, dec!(13_614.00));
        // min(13614 * 0.90, 12000) = 12000
        assert_eq!(result.required_annual_payment, dec!(12_000.00));
    }

    #[test]
    fn computed_values_maps_worksheet_fields() {
        let estimate = EstimatedTaxWorksheetResult {
            taxable_income: dec!(85_000.00),
            calculated_tax: dec!(13_614.00),
            total_estimated_tax: dec!(20_679.00),
            required_annual_payment: dec!(12_000.00),
            underpayment: dec!(12_000.00),
            estimated_payments_required: true,
        };

        let computed = computed_values(dec!(7_065.00), &estimate);

        assert_eq!(
            computed,
            TaxEstimateComputed {
                se_tax: dec!(7_065.00),
                total_tax: dec!(20_679.00),
                required_payment: dec!(12_000.00),
            }
        );
    }

    #[test]
    fn calculate_estimate_reports_unloaded_year() {
        let active = active_tax_year(Some(2024), vec![single_status_data()]);

        let Err(err) = calculate_estimate(&active, &wage_only_input(), Decimal::ZERO) else {
            panic!("calculation should fail when the active year differs");
        };

        assert!(matches!(
            err,
            EstimateError::TaxYearNotLoaded { year: 2025 }
        ));
    }

    #[test]
    fn calculate_estimate_reports_missing_filing_status() {
        let active = active_tax_year(Some(2025), Vec::new());

        let Err(err) = calculate_estimate(&active, &wage_only_input(), Decimal::ZERO) else {
            panic!("calculation should fail without bracket data");
        };

        assert!(matches!(
            err,
            EstimateError::MissingFilingStatus {
                year: 2025,
                status: FilingStatusCode::Single,
            }
        ));
    }

    #[test]
    fn calculate_estimate_matches_estimated_tax() {
        let active = active_tax_year(Some(2025), vec![single_status_data()]);

        let result = calculate_estimate(&active, &wage_only_input(), Decimal::ZERO)
            .expect("calculation should succeed with matching data loaded");

        assert_eq!(result.total_estimated_tax, dec!(13_614.00));
        assert_eq!(result.required_annual_payment, dec!(12_000.00));
    }
}
