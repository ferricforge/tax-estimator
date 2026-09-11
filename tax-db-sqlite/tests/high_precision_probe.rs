use std::str::FromStr;

use rust_decimal::Decimal;
use tax_core::{FilingStatusCode, TaxEstimateInput, TaxRepository};
use tax_db_sqlite::SqliteRepository;

#[tokio::test]
async fn create_returns_same_persisted_decimal_values_as_get() {
    let repo = SqliteRepository::new(":memory:").await.expect("repository");
    repo.run_migrations().await.expect("migrations");
    sqlx::query(
        "INSERT INTO filing_status (id, status_code, status_name) VALUES (1, 'S', 'Single')",
    )
    .execute(repo.pool())
    .await
    .expect("filing status");
    sqlx::query(
        "INSERT INTO tax_year_config (
            tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
            se_tax_deductible_percentage, se_deduction_factor,
            required_payment_threshold, min_se_threshold
        ) VALUES (9999, 1, 1, 1, 1, 1, 1, 1)",
    )
    .execute(repo.pool())
    .await
    .expect("tax year");

    let precise = Decimal::from_str("12345.6789012345678901234567").expect("decimal");
    let created = repo
        .create_estimate(TaxEstimateInput {
            tax_year: 9999,
            filing_status: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: precise,
            expected_deduction: Decimal::ZERO,
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        })
        .await
        .expect("create");
    let fetched = repo.get_estimate(created.id).await.expect("fetch");

    assert_eq!(created.input.expected_agi, fetched.input.expected_agi);
}
