//! Database-backed tests for the shared repository handle: a seeded in-memory
//! SQLite database is created, then read back through [`TaxRepo`].

use std::sync::Arc;

use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;
use tax_core::{
    FilingStatusCode, RepositoryError, TaxEstimateComputed, TaxEstimateInput, TaxRepository,
    db::DbConfig,
};
use tax_db_sqlite::{SqliteRepository, seeds};
use tax_ui::repository::{TaxRepo, build_registry};

async fn setup_test_repo() -> (Arc<dyn TaxRepository>, TaxRepo) {
    let sqlite_repo = SqliteRepository::new(":memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlite_repo
        .run_migrations()
        .await
        .expect("Failed to run migrations");

    sqlite_repo
        .run_seeds(seeds::embedded())
        .await
        .expect("Failed to run seeds");

    let repo: Arc<dyn TaxRepository> = Arc::new(sqlite_repo);
    let tax_repo = TaxRepo::new(repo.clone());

    (repo, tax_repo)
}

fn full_input() -> TaxEstimateInput {
    TaxEstimateInput {
        tax_year: 2025,
        filing_status: FilingStatusCode::Single,
        se_income: Some(dec!(50000.00)),
        expected_crp_payments: Some(dec!(5000.00)),
        expected_wages: Some(dec!(60000.00)),
        expected_agi: dec!(100000.00),
        expected_deduction: dec!(15000.00),
        expected_qbi_deduction: Some(dec!(5000.00)),
        expected_amt: Some(dec!(1000.00)),
        expected_credits: Some(dec!(2000.00)),
        expected_other_taxes: Some(dec!(500.00)),
        expected_withholding: Some(dec!(8000.00)),
        prior_year_tax: Some(dec!(12000.00)),
    }
}

fn minimal_input() -> TaxEstimateInput {
    TaxEstimateInput {
        tax_year: 2025,
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

#[tokio::test]
async fn get_estimate_loads_all_optional_fields() {
    let (repo, tax_repo) = setup_test_repo().await;
    let input = full_input();

    let created = repo
        .create_estimate(input)
        .await
        .expect("create_estimate should succeed");

    let loaded = tax_repo
        .get_estimate(created.id)
        .await
        .expect("get_estimate should succeed");

    assert_eq!(loaded, created);
}

#[tokio::test]
async fn get_estimate_loads_none_for_absent_optional_fields() {
    let (repo, tax_repo) = setup_test_repo().await;
    let input = minimal_input();

    let created = repo
        .create_estimate(input)
        .await
        .expect("create_estimate should succeed");

    let loaded = tax_repo
        .get_estimate(created.id)
        .await
        .expect("get_estimate should succeed");

    assert_eq!(loaded, created);
}

#[tokio::test]
async fn get_estimate_loads_persisted_computed_results() {
    let (repo, tax_repo) = setup_test_repo().await;
    let input = minimal_input();

    let mut estimate = repo
        .create_estimate(input)
        .await
        .expect("create_estimate should succeed");

    let expected_computed = TaxEstimateComputed {
        se_tax: dec!(7500.00),
        total_tax: dec!(25000.00),
        required_payment: dec!(4000.00),
    };

    estimate.computed = Some(expected_computed.clone());
    repo.update_estimate(&estimate)
        .await
        .expect("update_estimate should succeed");

    let loaded = tax_repo
        .get_estimate(estimate.id)
        .await
        .expect("get_estimate should succeed");

    assert_eq!(loaded.input, estimate.input);
    assert_eq!(loaded.computed, Some(expected_computed));
}

#[tokio::test]
async fn get_estimate_returns_not_found_for_missing_id() {
    let (_repo, tax_repo) = setup_test_repo().await;

    let result = tax_repo.get_estimate(99999).await;

    assert!(matches!(result, Err(RepositoryError::NotFound)));
}

#[tokio::test]
async fn list_estimates_with_and_without_year_filter() {
    let (repo, tax_repo) = setup_test_repo().await;

    let single_input = TaxEstimateInput {
        tax_year: 2025,
        filing_status: FilingStatusCode::Single,
        se_income: None,
        expected_crp_payments: None,
        expected_wages: None,
        expected_agi: dec!(80000.00),
        expected_deduction: dec!(15000.00),
        expected_qbi_deduction: None,
        expected_amt: None,
        expected_credits: None,
        expected_other_taxes: None,
        expected_withholding: None,
        prior_year_tax: None,
    };

    let mfj_input = TaxEstimateInput {
        tax_year: 2025,
        filing_status: FilingStatusCode::MarriedFilingJointly,
        se_income: Some(dec!(40000.00)),
        expected_crp_payments: None,
        expected_wages: None,
        expected_agi: dec!(120000.00),
        expected_deduction: dec!(30000.00),
        expected_qbi_deduction: None,
        expected_amt: None,
        expected_credits: None,
        expected_other_taxes: None,
        expected_withholding: None,
        prior_year_tax: None,
    };

    repo.create_estimate(single_input)
        .await
        .expect("create single estimate");
    repo.create_estimate(mfj_input)
        .await
        .expect("create MFJ estimate");

    let all = tax_repo
        .list_estimates(None)
        .await
        .expect("list all estimates");
    assert_eq!(all.len(), 2);

    let for_2025 = tax_repo
        .list_estimates(Some(2025))
        .await
        .expect("list estimates for 2025");
    assert_eq!(for_2025.len(), 2);
    for estimate in &for_2025 {
        assert_eq!(estimate.input.tax_year, 2025);
    }

    let for_2024 = tax_repo
        .list_estimates(Some(2024))
        .await
        .expect("list estimates for 2024");
    assert_eq!(for_2024.len(), 0);
}

#[tokio::test]
async fn registry_opens_the_sqlite_backend() {
    let db_config = DbConfig {
        backend: "sqlite".to_string(),
        connection_string: ":memory:".to_string(),
        ..Default::default()
    };

    let repo = build_registry().create(&db_config).await;

    assert!(repo.is_ok(), "the sqlite backend should be registered");
}

#[tokio::test]
async fn registry_rejects_an_unregistered_backend() {
    let db_config = DbConfig {
        backend: "not-a-backend".to_string(),
        connection_string: ":memory:".to_string(),
        ..Default::default()
    };

    let repo = build_registry().create(&db_config).await;

    assert!(repo.is_err(), "an unknown backend should not be created");
}
