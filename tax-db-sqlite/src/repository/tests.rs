use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;
use sqlx::sqlite::SqlitePoolOptions;
use tax_core::{
    FilingStatusCode, RepositoryError, TaxBracket, TaxEstimateComputed, TaxEstimateInput,
    TaxRepository,
};

use crate::seeds;

use super::SqliteRepository;

const TEST_YEAR: i32 = 9999;
const TEST_STATUS_ID: i32 = 99;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

async fn setup_test_db() -> SqliteRepository {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");
    let repo = SqliteRepository::new_with_pool(pool).await;
    repo.run_migrations()
        .await
        .expect("Failed to run migrations");
    repo
}

/// Remove every row from every table, honouring foreign-key order.
async fn clear_all_data(repo: &SqliteRepository) {
    for statement in [
        "DELETE FROM tax_estimate",
        "DELETE FROM standard_deductions",
        "DELETE FROM tax_brackets",
        "DELETE FROM filing_status",
        "DELETE FROM tax_year_config",
    ] {
        sqlx::query(statement)
            .execute(repo.pool())
            .await
            .unwrap_or_else(|e| panic!("Failed to run '{statement}': {e}"));
    }
}

async fn seed_tax_year_config(
    repo: &SqliteRepository,
    tax_year: i32,
) {
    sqlx::query(
        "INSERT INTO tax_year_config (
            tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
            se_tax_deductible_percentage, se_deduction_factor,
            required_payment_threshold, min_se_threshold
        ) VALUES (?, 200000.00, 0.125, 0.030, 0.9300, 0.55, 1500.00, 400.00)",
    )
    .bind(tax_year)
    .execute(repo.pool())
    .await
    .expect("Failed to insert tax year config");
}

async fn seed_filing_status(
    repo: &SqliteRepository,
    id: i32,
    status_code: &str,
    status_name: &str,
) {
    sqlx::query("INSERT INTO filing_status (id, status_code, status_name) VALUES (?, ?, ?)")
        .bind(id)
        .bind(status_code)
        .bind(status_name)
        .execute(repo.pool())
        .await
        .expect("Failed to insert filing status");
}

async fn seed_standard_deduction(
    repo: &SqliteRepository,
    tax_year: i32,
    filing_status_id: i32,
    amount: f64,
) {
    sqlx::query(
        "INSERT INTO standard_deductions (tax_year, filing_status_id, amount) VALUES (?, ?, ?)",
    )
    .bind(tax_year)
    .bind(filing_status_id)
    .bind(amount)
    .execute(repo.pool())
    .await
    .expect("Failed to insert standard deduction");
}

async fn seed_tax_brackets(
    repo: &SqliteRepository,
    tax_year: i32,
    filing_status_id: i32,
    brackets: &[(f64, Option<f64>, f64, f64)],
) {
    for &(min_income, max_income, tax_rate, base_tax) in brackets {
        sqlx::query(
            "INSERT INTO tax_brackets
                 (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .bind(min_income)
        .bind(max_income)
        .bind(tax_rate)
        .bind(base_tax)
        .execute(repo.pool())
        .await
        .expect("Failed to insert tax bracket");
    }
}

/// Reference data for the reference-table lookup tests: one tax-year config and
/// a "Single" filing status, both keyed on the test constants.
async fn setup_reference_data(repo: &SqliteRepository) {
    clear_all_data(repo).await;
    seed_tax_year_config(repo, TEST_YEAR).await;
    seed_filing_status(repo, TEST_STATUS_ID, "S", "Test Single").await;
}

/// Reference data for the estimate tests: two tax-year configs (8887/8888) and a
/// "Single" filing status (id 50) so `create_estimate` can resolve "S".
async fn setup_estimate_data(repo: &SqliteRepository) {
    clear_all_data(repo).await;
    seed_tax_year_config(repo, 8888).await;
    seed_tax_year_config(repo, 8887).await;
    seed_filing_status(repo, 50, "S", "Test Single").await;
}

fn estimate_input(tax_year: i32) -> TaxEstimateInput {
    TaxEstimateInput {
        tax_year,
        filing_status: FilingStatusCode::Single,
        se_income: None,
        expected_crp_payments: None,
        expected_wages: None,
        expected_agi: dec!(75000.00),
        expected_deduction: dec!(15000.00),
        expected_qbi_deduction: None,
        expected_amt: None,
        expected_credits: None,
        expected_other_taxes: None,
        expected_withholding: None,
        prior_year_tax: None,
    }
}

/// Every optional/monetary field populated, for full round-trip coverage.
fn full_estimate_input() -> TaxEstimateInput {
    TaxEstimateInput {
        expected_agi: dec!(100000.00),
        se_income: Some(dec!(50000.00)),
        expected_wages: Some(dec!(50000.00)),
        expected_qbi_deduction: Some(dec!(5000.00)),
        expected_credits: Some(dec!(2000.00)),
        expected_withholding: Some(dec!(8000.00)),
        prior_year_tax: Some(dec!(12000.00)),
        ..estimate_input(8888)
    }
}

fn assert_estimate_input_eq(
    actual: &TaxEstimateInput,
    expected: &TaxEstimateInput,
) {
    assert_eq!(actual.tax_year, expected.tax_year);
    assert_eq!(actual.filing_status, expected.filing_status);
    assert_eq!(actual.se_income, expected.se_income);
    assert_eq!(actual.expected_crp_payments, expected.expected_crp_payments);
    assert_eq!(actual.expected_wages, expected.expected_wages);
    assert_eq!(actual.expected_agi, expected.expected_agi);
    assert_eq!(actual.expected_deduction, expected.expected_deduction);
    assert_eq!(
        actual.expected_qbi_deduction,
        expected.expected_qbi_deduction
    );
    assert_eq!(actual.expected_amt, expected.expected_amt);
    assert_eq!(actual.expected_credits, expected.expected_credits);
    assert_eq!(actual.expected_other_taxes, expected.expected_other_taxes);
    assert_eq!(actual.expected_withholding, expected.expected_withholding);
    assert_eq!(actual.prior_year_tax, expected.prior_year_tax);
}

const SAMPLE_BRACKETS: [(f64, Option<f64>, f64, f64); 3] = [
    (0.0, Some(10000.0), 0.10, 0.0),
    (10000.0, Some(50000.0), 0.15, 1000.0),
    (50000.0, None, 0.25, 7000.0),
];

// ---------------------------------------------------------------------------
// `RepositoryError::NotFound` paths
// ---------------------------------------------------------------------------

macro_rules! not_found_test {
    ($name:ident, |$repo:ident| $call:expr) => {
        #[tokio::test]
        async fn $name() {
            let $repo = setup_test_db().await;
            assert!(matches!($call.await, Err(RepositoryError::NotFound)));
        }
    };
}

not_found_test!(get_tax_year_config_missing_year_is_not_found, |repo| repo
    .get_tax_year_config(1999));
not_found_test!(get_filing_status_missing_id_is_not_found, |repo| repo
    .get_filing_status(999));
not_found_test!(
    get_filing_status_by_code_unknown_code_is_not_found,
    |repo| repo.get_filing_status_by_code("INVALID")
);
not_found_test!(get_standard_deduction_missing_is_not_found, |repo| repo
    .get_standard_deduction(1999, 1));
not_found_test!(get_estimate_missing_id_is_not_found, |repo| repo
    .get_estimate(99999));
not_found_test!(delete_estimate_missing_id_is_not_found, |repo| repo
    .delete_estimate(99999));

// ---------------------------------------------------------------------------
// tax_year_config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_tax_year_config_returns_all_fields() {
    let repo = setup_test_db().await;
    seed_tax_year_config(&repo, TEST_YEAR).await;

    let config = repo
        .get_tax_year_config(TEST_YEAR)
        .await
        .expect("Should find test config");

    assert_eq!(config.tax_year, TEST_YEAR);
    assert_eq!(config.ss_wage_max, dec!(200000.00));
    assert_eq!(config.ss_tax_rate, dec!(0.125));
    assert_eq!(config.medicare_tax_rate, dec!(0.030));
    assert_eq!(config.se_tax_deduct_pcnt, dec!(0.9300));
    assert_eq!(config.se_deduction_factor, dec!(0.55));
    assert_eq!(config.req_pmnt_threshold, dec!(1500.00));
    assert_eq!(config.min_se_threshold, dec!(400.00));
}

#[tokio::test]
async fn list_tax_years_includes_seeded_year() {
    let repo = setup_test_db().await;
    seed_tax_year_config(&repo, TEST_YEAR).await;

    let years = repo.list_tax_years().await.expect("Should list tax years");

    assert!(years.contains(&TEST_YEAR));
}

// ---------------------------------------------------------------------------
// filing_status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_filing_status_by_id_returns_row() {
    let repo = setup_test_db().await;
    clear_all_data(&repo).await;
    seed_filing_status(&repo, 42, "HOH", "Test Head of Household").await;

    let status = repo
        .get_filing_status(42)
        .await
        .expect("Should find test filing status");

    assert_eq!(status.id, 42);
    assert_eq!(status.status_code, FilingStatusCode::HeadOfHousehold);
    assert_eq!(status.status_name, "Test Head of Household");
}

#[tokio::test]
async fn get_filing_status_by_code_returns_row() {
    let repo = setup_test_db().await;
    clear_all_data(&repo).await;
    seed_filing_status(&repo, 7, "MFJ", "Test Married Filing Jointly").await;

    let status = repo
        .get_filing_status_by_code("MFJ")
        .await
        .expect("Should find filing status by code");

    assert_eq!(status.id, 7);
    assert_eq!(status.status_code, FilingStatusCode::MarriedFilingJointly);
    assert_eq!(status.status_name, "Test Married Filing Jointly");
}

#[tokio::test]
async fn list_filing_statuses_returns_rows_ordered_by_id() {
    let repo = setup_test_db().await;
    clear_all_data(&repo).await;
    seed_filing_status(&repo, 20, "MFJ", "Test Married Filing Jointly").await;
    seed_filing_status(&repo, 10, "S", "Test Single").await;

    let statuses = repo
        .list_filing_statuses()
        .await
        .expect("Should list filing statuses");

    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].id, 10);
    assert_eq!(statuses[0].status_code, FilingStatusCode::Single);
    assert_eq!(statuses[0].status_name, "Test Single");
    assert_eq!(statuses[1].id, 20);
    assert_eq!(
        statuses[1].status_code,
        FilingStatusCode::MarriedFilingJointly
    );
    assert_eq!(statuses[1].status_name, "Test Married Filing Jointly");
}

// ---------------------------------------------------------------------------
// standard_deductions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_standard_deduction_returns_row() {
    let repo = setup_test_db().await;
    setup_reference_data(&repo).await;
    seed_standard_deduction(&repo, TEST_YEAR, TEST_STATUS_ID, 18000.00).await;

    let deduction = repo
        .get_standard_deduction(TEST_YEAR, TEST_STATUS_ID)
        .await
        .expect("Should find test standard deduction");

    assert_eq!(deduction.tax_year, TEST_YEAR);
    assert_eq!(deduction.filing_status_id, TEST_STATUS_ID);
    assert_eq!(deduction.amount, dec!(18000.00));
}

// ---------------------------------------------------------------------------
// tax_brackets
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_tax_brackets_returns_rows_ordered_by_min_income() {
    let repo = setup_test_db().await;
    setup_reference_data(&repo).await;
    seed_tax_brackets(&repo, TEST_YEAR, TEST_STATUS_ID, &SAMPLE_BRACKETS).await;

    let brackets = repo
        .get_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
        .await
        .expect("Should find test tax brackets");

    assert_eq!(brackets.len(), 3);

    assert_eq!(brackets[0].min_income, dec!(0));
    assert_eq!(brackets[0].max_income, Some(dec!(10000)));
    assert_eq!(brackets[0].tax_rate, dec!(0.10));
    assert_eq!(brackets[0].base_tax, dec!(0));

    assert_eq!(brackets[1].min_income, dec!(10000));
    assert_eq!(brackets[1].max_income, Some(dec!(50000)));
    assert_eq!(brackets[1].tax_rate, dec!(0.15));
    assert_eq!(brackets[1].base_tax, dec!(1000));

    assert_eq!(brackets[2].min_income, dec!(50000));
    assert_eq!(brackets[2].max_income, None);
    assert_eq!(brackets[2].tax_rate, dec!(0.25));
    assert_eq!(brackets[2].base_tax, dec!(7000));
}

#[tokio::test]
async fn get_tax_brackets_returns_empty_when_nothing_matches() {
    let repo = setup_test_db().await;

    let brackets = repo
        .get_tax_brackets(1999, 1)
        .await
        .expect("Should return empty vec");

    assert!(brackets.is_empty());
}

macro_rules! insert_bracket_roundtrip {
    ($name:ident, max_income = $max:expr) => {
        #[tokio::test]
        async fn $name() {
            let repo = setup_test_db().await;
            setup_reference_data(&repo).await;

            let bracket = TaxBracket {
                tax_year: TEST_YEAR,
                filing_status_id: TEST_STATUS_ID,
                min_income: dec!(100000),
                max_income: $max,
                tax_rate: dec!(0.37),
                base_tax: dec!(25000),
            };
            repo.insert_tax_bracket(&bracket)
                .await
                .expect("Should insert bracket");

            let brackets = repo
                .get_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
                .await
                .expect("Should get brackets");

            assert_eq!(brackets.len(), 1);
            assert_eq!(brackets[0].tax_year, TEST_YEAR);
            assert_eq!(brackets[0].filing_status_id, TEST_STATUS_ID);
            assert_eq!(brackets[0].min_income, dec!(100000));
            assert_eq!(brackets[0].max_income, $max);
            assert_eq!(brackets[0].tax_rate, dec!(0.37));
            assert_eq!(brackets[0].base_tax, dec!(25000));
        }
    };
}

insert_bracket_roundtrip!(
    insert_tax_bracket_round_trips_bounded_max,
    max_income = Some(dec!(200000))
);
insert_bracket_roundtrip!(
    insert_tax_bracket_round_trips_open_ended_max,
    max_income = None
);

#[tokio::test]
async fn delete_tax_brackets_removes_every_row_for_group() {
    let repo = setup_test_db().await;
    setup_reference_data(&repo).await;
    seed_tax_brackets(&repo, TEST_YEAR, TEST_STATUS_ID, &SAMPLE_BRACKETS).await;
    assert_eq!(
        repo.get_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
            .await
            .expect("Should get brackets")
            .len(),
        3,
    );

    repo.delete_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
        .await
        .expect("Should delete brackets");

    assert!(
        repo.get_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
            .await
            .expect("Should get brackets")
            .is_empty()
    );
}

#[tokio::test]
async fn delete_tax_brackets_is_ok_when_nothing_matches() {
    let repo = setup_test_db().await;

    repo.delete_tax_brackets(TEST_YEAR, TEST_STATUS_ID)
        .await
        .expect("Should succeed even if no brackets exist");
}

// ---------------------------------------------------------------------------
// tax_estimate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_estimate_persists_every_input_field_and_leaves_computed_empty() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;

    let created = repo
        .create_estimate(full_estimate_input())
        .await
        .expect("Should create estimate");

    assert!(created.id > 0);
    assert_estimate_input_eq(&created.input, &full_estimate_input());
    assert_eq!(created.computed, None);
}

#[tokio::test]
async fn get_estimate_returns_previously_created_row() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;
    let created = repo
        .create_estimate(full_estimate_input())
        .await
        .expect("Should create estimate");

    let fetched = repo
        .get_estimate(created.id)
        .await
        .expect("Should fetch estimate");

    assert_eq!(fetched.id, created.id);
    assert_estimate_input_eq(&fetched.input, &full_estimate_input());
    assert_eq!(fetched.computed, None);
}

#[tokio::test]
async fn create_estimate_upserts_on_year_and_filing_status_conflict() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;

    let first = repo
        .create_estimate(TaxEstimateInput {
            expected_agi: dec!(100000.00),
            ..estimate_input(8888)
        })
        .await
        .expect("Should create estimate");
    let second = repo
        .create_estimate(TaxEstimateInput {
            expected_agi: dec!(120000.00),
            ..estimate_input(8888)
        })
        .await
        .expect("Should upsert estimate");

    assert_eq!(
        first.id, second.id,
        "same tax year and filing status should update the existing row"
    );
    assert_eq!(second.input.expected_agi, dec!(120000.00));
    assert_eq!(
        repo.list_estimates(None)
            .await
            .expect("Should list estimates")
            .len(),
        1,
    );
}

#[tokio::test]
async fn update_estimate_persists_input_and_computed_changes() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;

    let mut estimate = repo
        .create_estimate(estimate_input(8888))
        .await
        .expect("Should create estimate");
    estimate.input.expected_agi = dec!(150000.00);
    estimate.input.expected_deduction = dec!(18000.00);
    estimate.computed = Some(TaxEstimateComputed {
        se_tax: dec!(7500.00),
        total_tax: dec!(25000.00),
        required_payment: dec!(4000.00),
    });

    repo.update_estimate(&estimate)
        .await
        .expect("Should update estimate");

    let fetched = repo
        .get_estimate(estimate.id)
        .await
        .expect("Should fetch estimate");

    assert_eq!(fetched.input.expected_agi, dec!(150000.00));
    assert_eq!(fetched.input.expected_deduction, dec!(18000.00));
    assert_eq!(
        fetched.computed,
        Some(TaxEstimateComputed {
            se_tax: dec!(7500.00),
            total_tax: dec!(25000.00),
            required_payment: dec!(4000.00),
        })
    );
}

#[tokio::test]
async fn update_estimate_unknown_id_is_not_found() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;

    let mut estimate = repo
        .create_estimate(estimate_input(8888))
        .await
        .expect("Should create estimate");
    estimate.id = 99999;

    assert!(matches!(
        repo.update_estimate(&estimate).await,
        Err(RepositoryError::NotFound)
    ));
}

#[tokio::test]
async fn delete_estimate_removes_the_row() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;

    let created = repo
        .create_estimate(estimate_input(8888))
        .await
        .expect("Should create estimate");

    repo.delete_estimate(created.id)
        .await
        .expect("Should delete estimate");

    assert!(matches!(
        repo.get_estimate(created.id).await,
        Err(RepositoryError::NotFound)
    ));
}

#[tokio::test]
async fn list_estimates_returns_all_rows_when_year_is_none() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;
    repo.create_estimate(estimate_input(8888))
        .await
        .expect("Should create 8888 estimate");
    repo.create_estimate(estimate_input(8887))
        .await
        .expect("Should create 8887 estimate");

    let all = repo
        .list_estimates(None)
        .await
        .expect("Should list all estimates");

    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn list_estimates_filters_by_requested_year() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;
    repo.create_estimate(estimate_input(8888))
        .await
        .expect("Should create 8888 estimate");
    repo.create_estimate(estimate_input(8887))
        .await
        .expect("Should create 8887 estimate");

    let for_8888 = repo
        .list_estimates(Some(8888))
        .await
        .expect("Should list for 8888");

    assert_eq!(for_8888.len(), 1);
    assert_eq!(for_8888[0].input.tax_year, 8888);
}

#[tokio::test]
async fn list_estimates_returns_empty_for_year_without_rows() {
    let repo = setup_test_db().await;
    setup_estimate_data(&repo).await;
    repo.create_estimate(estimate_input(8888))
        .await
        .expect("Should create 8888 estimate");

    let for_7777 = repo
        .list_estimates(Some(7777))
        .await
        .expect("Should list for 7777");

    assert!(for_7777.is_empty());
}

// ---------------------------------------------------------------------------
// Seeds
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_seeds_populates_every_reference_table() {
    let repo = setup_test_db().await;
    clear_all_data(&repo).await;

    repo.run_seeds(seeds::embedded())
        .await
        .expect("Should run seeds successfully");

    assert_eq!(
        repo.list_filing_statuses()
            .await
            .expect("Should list filing statuses")
            .len(),
        5,
    );

    let config = repo
        .get_tax_year_config(2025)
        .await
        .expect("Should find 2025 config");
    assert_eq!(config.tax_year, 2025);

    let deduction = repo
        .get_standard_deduction(2025, 1)
        .await
        .expect("Should find standard deduction");
    assert_eq!(deduction.tax_year, 2025);
    assert_eq!(deduction.filing_status_id, 1);

    assert_eq!(
        repo.get_tax_brackets(2025, 1)
            .await
            .expect("Should find tax brackets")
            .len(),
        7,
    );
}

#[tokio::test]
async fn run_seeds_errors_for_missing_directory() {
    let repo = setup_test_db().await;

    let err = repo
        .run_seeds(seeds::embedded())
        .await
        .expect_err("Should fail for nonexistent directory");

    assert_eq!(
        err.to_string(),
        "Failed to read seeds directory './nonexistent'"
    );
}
