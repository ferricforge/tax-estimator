//! Integration test: EstimatedIncomeModel → DbConfig → SE Tax → TaxEstimate.
//!
//! Demonstrates building a model, connecting via DbConfig, loading reference
//! data, running tax-core SE and Estimated Tax worksheet calculations, and
//! persisting the final TaxEstimate with calculated fields.

use rust_decimal::Decimal;
use tax_core::FilingStatusCode;
use tax_core::calculations::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetInput, SeWorksheet, SeWorksheetConfig,
};
use tax_core::db::DbConfig;
use tax_ui::app::{FilingStatusData, TaxYearData, build_registry, load_tax_year_data};
use tax_ui::models::EstimatedIncomeModel;

use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;

fn make_model() -> EstimatedIncomeModel {
    EstimatedIncomeModel {
        tax_year: 2025,
        filing_status_id: FilingStatusCode::Single,
        se_income: Some(dec!(100_000.00)),
        expected_crp_payments: None,
        expected_wages: Some(dec!(50_000.00)),
        expected_agi: dec!(175_000.00),
        expected_deduction: dec!(0), // use standard from repo
        expected_qbi_deduction: None,
        expected_amt: None,
        expected_credits: None,
        expected_other_taxes: None,
        expected_withholding: Some(dec!(20_000.00)),
        prior_year_tax: Some(dec!(25_000.00)),
    }
}

fn status_data_for_filing_status_id(
    data: &TaxYearData,
    filing_status_id: i32,
) -> Option<&FilingStatusData> {
    data.statuses
        .iter()
        .find(|s| s.filing_status.id == filing_status_id)
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
    model: &EstimatedIncomeModel,
    se_self_employment_tax: Decimal,
    config: &tax_core::TaxYearConfig,
) -> tax_core::calculations::EstimatedTaxWorksheetResult {
    let input = EstimatedTaxWorksheetInput {
        adjusted_gross_income: model.expected_agi,
        itemized_deduction: model.expected_deduction,
        standard_deduction: status_data.standard_deduction.amount,
        qbi_deduction: model.expected_qbi_deduction.unwrap_or(Decimal::ZERO),
        alternative_minimum_tax: model.expected_amt.unwrap_or(Decimal::ZERO),
        credits: model.expected_credits.unwrap_or(Decimal::ZERO),
        self_employment_tax: se_self_employment_tax,
        other_taxes: model.expected_other_taxes.unwrap_or(Decimal::ZERO),
        refundable_credits: Decimal::ZERO,
        prior_year_tax: model.prior_year_tax.unwrap_or(Decimal::ZERO),
        withholding: model.expected_withholding.unwrap_or(Decimal::ZERO),
        is_farmer_or_fisher: false,
        required_payment_threshold: config.required_payment_threshold,
    };
    let worksheet = EstimatedTaxWorksheet::new(&status_data.tax_brackets);
    worksheet
        .calculate(&input)
        .expect("Estimated tax worksheet calculation should succeed")
}

#[tokio::test]
async fn estimate_model_through_db_and_calculations_to_tax_estimate() {
    let model = make_model();
    let new_est = model.to_new_tax_estimate();

    let db_config = DbConfig {
        backend: "sqlite".to_string(),
        connection_string: ":memory:".to_string(),
    };
    let registry = build_registry();
    let repo = registry
        .create(&db_config)
        .await
        .expect("repository creation should succeed");

    let year_data = load_tax_year_data(&*repo, model.tax_year)
        .await
        .expect("load_tax_year_data should succeed");

    let status_data = status_data_for_filing_status_id(&year_data, new_est.filing_status_id)
        .expect("seeded DB should have filing status for estimate");

    let se_income = model.se_income.unwrap_or(Decimal::ZERO);
    let crp = model.expected_crp_payments.unwrap_or(Decimal::ZERO);
    let wages = model.expected_wages.unwrap_or(Decimal::ZERO);
    let se_result = run_se_worksheet(&year_data.config, se_income, crp, wages);

    let est_result = run_estimated_tax_worksheet(
        status_data,
        &model,
        se_result.self_employment_tax,
        &year_data.config,
    );

    let created = repo
        .create_estimate(new_est.clone())
        .await
        .expect("create_estimate should succeed");

    let mut updated = created.clone();
    updated.calculated_se_tax = Some(se_result.self_employment_tax);
    updated.calculated_total_tax = Some(est_result.total_estimated_tax);
    updated.calculated_required_payment = Some(est_result.required_annual_payment);

    repo.update_estimate(&updated)
        .await
        .expect("update_estimate should succeed");

    let fetched = repo
        .get_estimate(created.id)
        .await
        .expect("get_estimate should succeed");

    assert_eq!(
        fetched.calculated_se_tax,
        Some(se_result.self_employment_tax),
        "calculated_se_tax should match SE worksheet"
    );
    assert_eq!(
        fetched.calculated_total_tax,
        Some(est_result.total_estimated_tax),
        "calculated_total_tax should match estimated tax worksheet"
    );
    assert_eq!(
        fetched.calculated_required_payment,
        Some(est_result.required_annual_payment),
        "calculated_required_payment should match estimated tax worksheet"
    );
    assert_eq!(fetched.tax_year, model.tax_year);
    assert_eq!(fetched.filing_status_id, new_est.filing_status_id);
    assert_eq!(fetched.expected_agi, model.expected_agi);
    assert_eq!(fetched.expected_deduction, model.expected_deduction);
}
