//! Integration test: TaxEstimateInput → DbConfig → SE Tax → TaxEstimate.
//!
//! Demonstrates building canonical estimate input, loading reference data,
//! running both worksheets, and persisting the resulting estimate record.

use pretty_assertions::assert_eq;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tax_core::calculations::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetContext, EstimatedTaxWorksheetResult, SeWorksheet,
    SeWorksheetConfig, SeWorksheetResult,
};
use tax_core::{FilingStatusCode, TaxEstimate, TaxEstimateComputed, TaxEstimateInput};
use tax_db::{DbConfig, TaxRepository, open};
use tax_ui::app::{FilingStatusData, TaxYearData, load_tax_year_data};

fn make_input() -> TaxEstimateInput {
    TaxEstimateInput {
        tax_year: 2025,
        filing_status: FilingStatusCode::Single,
        se_income: Some(dec!(100_000.00)),
        expected_crp_payments: None,
        expected_wages: Some(dec!(50_000.00)),
        expected_agi: dec!(175_000.00),
        expected_deduction: dec!(15_000.00),
        expected_qbi_deduction: None,
        expected_amt: None,
        expected_credits: None,
        expected_other_taxes: None,
        expected_withholding: Some(dec!(20_000.00)),
        prior_year_tax: Some(dec!(25_000.00)),
    }
}

fn status_data_for_filing_status(
    data: &TaxYearData,
    filing_status: FilingStatusCode,
) -> Option<&FilingStatusData> {
    data.statuses
        .iter()
        .find(|s| s.filing_status.status_code == filing_status)
}

fn run_se_worksheet(
    config: &tax_core::TaxYearConfig,
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

fn run_estimated_tax_worksheet(
    status_data: &FilingStatusData,
    input: &TaxEstimateInput,
    se_self_employment_tax: Decimal,
    config: &tax_core::TaxYearConfig,
) -> tax_core::calculations::EstimatedTaxWorksheetResult {
    let worksheet_input = input.to_estimated_tax_worksheet_input(&EstimatedTaxWorksheetContext {
        self_employment_tax: se_self_employment_tax,
        refundable_credits: Decimal::ZERO,
        is_farmer_or_fisher: false,
        required_payment_threshold: config.req_pmnt_threshold,
    });
    let worksheet = EstimatedTaxWorksheet::new(&status_data.tax_brackets);
    worksheet
        .calculate(&worksheet_input)
        .expect("Estimated tax worksheet calculation should succeed")
}

#[tokio::test]
async fn estimate_input_through_db_and_calculations_to_tax_estimate() {
    let input = make_input();

    let db_config = DbConfig {
        backend: "sqlite".to_string(),
        connection_string: ":memory:".to_string(),
    };
    let repo = open(&db_config)
        .await
        .expect("repository creation should succeed");

    let year_data: TaxYearData = load_tax_year_data(&repo, input.tax_year)
        .await
        .expect("load_tax_year_data should succeed");

    let status_data: &FilingStatusData =
        status_data_for_filing_status(&year_data, input.filing_status)
            .expect("seeded DB should have filing status for estimate");

    let se_income: Decimal = input.se_income.unwrap_or(Decimal::ZERO);
    let crp: Decimal = input.expected_crp_payments.unwrap_or(Decimal::ZERO);
    let wages: Decimal = input.expected_wages.unwrap_or(Decimal::ZERO);
    let se_result: SeWorksheetResult = run_se_worksheet(&year_data.config, se_income, crp, wages);

    let est_result: EstimatedTaxWorksheetResult = run_estimated_tax_worksheet(
        status_data,
        &input,
        se_result.self_employment_tax,
        &year_data.config,
    );

    let created: TaxEstimate = TaxRepository::create::<TaxEstimate>(&repo, input.clone())
        .await
        .expect("create should succeed");

    let mut updated: TaxEstimate = created.clone();
    updated.computed = Some(TaxEstimateComputed {
        se_tax: se_result.self_employment_tax,
        total_tax: est_result.total_estimated_tax,
        required_payment: est_result.required_annual_payment,
    });

    TaxRepository::update(&repo, &updated)
        .await
        .expect("update should succeed");

    let fetched: TaxEstimate = TaxRepository::get::<TaxEstimate>(&repo, &created.id)
        .await
        .expect("get should succeed");

    assert_eq!(
        fetched.computed,
        Some(TaxEstimateComputed {
            se_tax: se_result.self_employment_tax,
            total_tax: est_result.total_estimated_tax,
            required_payment: est_result.required_annual_payment,
        }),
        "computed tax values should match both worksheets"
    );
    assert_eq!(fetched.input.tax_year, input.tax_year);
    assert_eq!(fetched.input.filing_status, input.filing_status);
    assert_eq!(fetched.input.expected_agi, input.expected_agi);
    assert_eq!(fetched.input.expected_deduction, input.expected_deduction);
}
