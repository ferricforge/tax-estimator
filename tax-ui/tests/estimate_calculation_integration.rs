//! Integration test: TaxEstimateInput → DbConfig → worksheets → TaxEstimate.
//!
//! Builds canonical estimate input, loads reference data from a seeded
//! in-memory SQLite database, runs both worksheets through the `tax_ui`
//! glue layer, persists the estimate, and reads it back.

use pretty_assertions::assert_eq;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tax_core::db::DbConfig;
use tax_core::{FilingStatusCode, TaxEstimate, TaxEstimateInput, TaxRepository};
use tax_ui::estimate::{computed_values, estimated_tax, save_tax_estimate, se_tax_estimate};
use tax_ui::models::TaxYearData;
use tax_ui::repository::build_registry;

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

#[tokio::test]
async fn estimate_input_through_db_and_calculations_to_tax_estimate() {
    let input = make_input();

    // ── repository ──────────────────────────────────────────────────────
    let db_config = DbConfig {
        backend: "sqlite".to_string(),
        connection_string: ":memory:".to_string(),
    };
    let repo: Box<dyn TaxRepository> = build_registry()
        .create(&db_config)
        .await
        .expect("repository creation should succeed");

    // ── reference data ──────────────────────────────────────────────────
    let year_data = TaxYearData::load(repo.as_ref(), input.tax_year)
        .await
        .expect("TaxYearData::load should succeed");
    let status = year_data
        .status_for(input.filing_status)
        .expect("seeded DB should have filing status for estimate");

    // ── worksheets ──────────────────────────────────────────────────────
    let se_result = se_tax_estimate(
        &year_data.config,
        input.se_income.unwrap_or(Decimal::ZERO),
        input.expected_crp_payments.unwrap_or(Decimal::ZERO),
        input.expected_wages.unwrap_or(Decimal::ZERO),
    )
    .expect("SE worksheet should succeed");

    let est_result = estimated_tax(
        status,
        &year_data.config,
        &input,
        se_result.self_employment_tax,
    )
    .expect("estimated tax worksheet should succeed");

    // ── persistence ─────────────────────────────────────────────────────
    let saved: TaxEstimate = save_tax_estimate(
        repo.as_ref(),
        &input,
        computed_values(se_result.self_employment_tax, &est_result),
    )
    .await
    .expect("save_tax_estimate should succeed");

    let fetched: TaxEstimate = repo
        .get_estimate(saved.id)
        .await
        .expect("get_estimate should succeed");

    // ── assertions ──────────────────────────────────────────────────────
    assert_eq!(
        fetched.computed,
        Some(computed_values(se_result.self_employment_tax, &est_result)),
        "computed tax values should match both worksheets"
    );
    assert_eq!(
        fetched.computed, saved.computed,
        "save_tax_estimate should return what was persisted"
    );
    assert_eq!(fetched.input.tax_year, input.tax_year);
    assert_eq!(fetched.input.filing_status, input.filing_status);
    assert_eq!(fetched.input.expected_agi, input.expected_agi);
    assert_eq!(fetched.input.expected_deduction, input.expected_deduction);
}
