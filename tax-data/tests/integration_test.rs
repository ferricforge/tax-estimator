//! Integration tests for tax bracket loading using actual database backend.

use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;
use sqlx::sqlite::SqlitePoolOptions;
use tax_core::TaxRepository;
use tax_data::{TaxBracketLoader, TaxBracketLoaderError};
use tax_db_sqlite::SqliteRepository;

const TEST_CSV_2025: &str = include_str!("../test-data/tax_brackets_2025.csv");

/// Sets up a test database with migrations run but NO seed data.
/// This simulates a user running --migrate without --seeds.
async fn setup_test_db_without_seeds() -> SqliteRepository {
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

async fn setup_test_db() -> SqliteRepository {
    let repo = setup_test_db_without_seeds().await;

    // Insert filing statuses (required for foreign key constraints)
    sqlx::query(
        "INSERT INTO filing_status (id, status_code, status_name) VALUES
         (1, 'S', 'Single'),
         (2, 'MFJ', 'Married Filing Jointly'),
         (3, 'MFS', 'Married Filing Separately'),
         (4, 'HOH', 'Head of Household'),
         (5, 'QSS', 'Qualifying Surviving Spouse')",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert filing statuses");

    // Insert tax year config (required for foreign key constraints)
    sqlx::query(
        "INSERT INTO tax_year_config (
            tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
            se_tax_deductible_percentage, se_deduction_factor, required_payment_threshold
        ) VALUES (2025, 176100, 0.062, 0.0145, 0.9235, 0.5, 1000)",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert tax year config");

    repo
}

#[tokio::test]
async fn test_load_all_2025_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    // 28 records in CSV, but Y-1 maps to both MFJ and QSS, so 28 + 7 = 35
    let inserted = TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    assert_eq!(inserted, 35);
}

#[tokio::test]
async fn test_load_and_retrieve_single_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    let brackets = repo
        .get_tax_brackets(2025, 1)
        .await
        .expect("Failed to get Single brackets");

    assert_eq!(brackets.len(), 7);

    // Verify first bracket (10%)
    assert_eq!(brackets[0].tax_year, 2025);
    assert_eq!(brackets[0].filing_status_id, 1);
    assert_eq!(brackets[0].min_income, dec!(0));
    assert_eq!(brackets[0].max_income, Some(dec!(11925)));
    assert_eq!(brackets[0].base_tax, dec!(0));
    assert_eq!(brackets[0].tax_rate, dec!(0.10));

    // Verify second bracket (12%)
    assert_eq!(brackets[1].min_income, dec!(11925));
    assert_eq!(brackets[1].max_income, Some(dec!(48475)));
    assert_eq!(brackets[1].base_tax, dec!(1192.50));
    assert_eq!(brackets[1].tax_rate, dec!(0.12));

    // Verify last bracket (37%, unlimited)
    assert_eq!(brackets[6].min_income, dec!(626350));
    assert_eq!(brackets[6].max_income, None);
    assert_eq!(brackets[6].base_tax, dec!(188769.75));
    assert_eq!(brackets[6].tax_rate, dec!(0.37));
}

#[tokio::test]
async fn test_load_and_retrieve_mfj_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    let brackets = repo
        .get_tax_brackets(2025, 2)
        .await
        .expect("Failed to get MFJ brackets");

    assert_eq!(brackets.len(), 7);

    // Verify first bracket
    assert_eq!(brackets[0].min_income, dec!(0));
    assert_eq!(brackets[0].max_income, Some(dec!(23850)));
    assert_eq!(brackets[0].base_tax, dec!(0));
    assert_eq!(brackets[0].tax_rate, dec!(0.10));

    // Verify last bracket
    assert_eq!(brackets[6].min_income, dec!(751600));
    assert_eq!(brackets[6].max_income, None);
    assert_eq!(brackets[6].base_tax, dec!(202154.50));
    assert_eq!(brackets[6].tax_rate, dec!(0.37));
}

#[tokio::test]
async fn test_load_and_retrieve_mfs_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    let brackets = repo
        .get_tax_brackets(2025, 3)
        .await
        .expect("Failed to get MFS brackets");

    assert_eq!(brackets.len(), 7);

    // MFS differs from Single in 35% bracket max (375800 vs 626350)
    let bracket_35 = brackets.iter().find(|b| b.tax_rate == dec!(0.35)).unwrap();
    assert_eq!(bracket_35.max_income, Some(dec!(375800)));

    // Verify last bracket
    assert_eq!(brackets[6].min_income, dec!(375800));
    assert_eq!(brackets[6].base_tax, dec!(101077.25));
    assert_eq!(brackets[6].tax_rate, dec!(0.37));
}

#[tokio::test]
async fn test_load_and_retrieve_hoh_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    let brackets = repo
        .get_tax_brackets(2025, 4)
        .await
        .expect("Failed to get HOH brackets");

    assert_eq!(brackets.len(), 7);

    // HOH has unique first bracket
    assert_eq!(brackets[0].min_income, dec!(0));
    assert_eq!(brackets[0].max_income, Some(dec!(17000)));

    // Second bracket
    assert_eq!(brackets[1].min_income, dec!(17000));
    assert_eq!(brackets[1].max_income, Some(dec!(64850)));
    assert_eq!(brackets[1].base_tax, dec!(1700.00));

    // Last bracket
    assert_eq!(brackets[6].min_income, dec!(626350));
    assert_eq!(brackets[6].base_tax, dec!(187031.50));
}

#[tokio::test]
async fn test_load_and_retrieve_qss_brackets() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    let qss_brackets = repo
        .get_tax_brackets(2025, 5)
        .await
        .expect("Failed to get QSS brackets");
    let mfj_brackets = repo
        .get_tax_brackets(2025, 2)
        .await
        .expect("Failed to get MFJ brackets");

    // QSS should match MFJ (both from Schedule Y-1)
    assert_eq!(qss_brackets.len(), mfj_brackets.len());

    for (qss, mfj) in qss_brackets.iter().zip(mfj_brackets.iter()) {
        assert_eq!(qss.min_income, mfj.min_income);
        assert_eq!(qss.max_income, mfj.max_income);
        assert_eq!(qss.base_tax, mfj.base_tax);
        assert_eq!(qss.tax_rate, mfj.tax_rate);
    }
}

#[tokio::test]
async fn test_load_is_idempotent() {
    let repo = setup_test_db().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");

    // Load twice
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("First load failed");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Second load failed");

    // Should still have exactly 7 brackets per filing status
    for status_id in 1..=5 {
        let brackets = repo
            .get_tax_brackets(2025, status_id)
            .await
            .expect("Failed to get brackets");
        assert_eq!(
            brackets.len(),
            7,
            "Expected 7 brackets for status_id {}",
            status_id
        );
    }
}

#[tokio::test]
async fn test_load_replaces_existing_brackets() {
    let repo = setup_test_db().await;

    // Insert some existing brackets
    sqlx::query(
        "INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
         VALUES (2025, 1, 0, 5000, 0.05, 0)",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert initial bracket");

    let initial_brackets = repo
        .get_tax_brackets(2025, 1)
        .await
        .expect("Failed to get initial brackets");
    assert_eq!(initial_brackets.len(), 1);
    assert_eq!(initial_brackets[0].max_income, Some(dec!(5000)));

    // Load the CSV data
    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    // Should now have the correct brackets
    let loaded_brackets = repo
        .get_tax_brackets(2025, 1)
        .await
        .expect("Failed to get loaded brackets");
    assert_eq!(loaded_brackets.len(), 7);
    assert_eq!(loaded_brackets[0].max_income, Some(dec!(11925)));
}

#[tokio::test]
async fn test_load_invalid_schedule() {
    let repo = setup_test_db().await;

    let csv = "tax_year,schedule,min_income,max_income,base_tax,rate\n2025,INVALID,0,10000,0,0.10";
    let records = TaxBracketLoader::parse(csv.as_bytes()).expect("Failed to parse CSV");

    let result = TaxBracketLoader::load(&repo, &records).await;

    assert_eq!(
        result,
        Err(TaxBracketLoaderError::InvalidSchedule(
            "INVALID".to_string()
        ))
    );
}

#[tokio::test]
async fn test_load_fails_without_filing_statuses() {
    let repo = setup_test_db_without_seeds().await;

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");

    let result = TaxBracketLoader::load(&repo, &records).await;

    // The exact status code depends on which schedule is processed first (HashMap ordering),
    // but we know it must be one of S, MFJ, MFS, HOH, or QSS
    let err = result.expect_err("Should fail when filing statuses are missing");
    let TaxBracketLoaderError::FilingStatusNotFound(code) = err else {
        panic!("Expected FilingStatusNotFound error, got: {:?}", err);
    };
    assert!(
        ["S", "MFJ", "MFS", "HOH", "QSS"].contains(&code.as_str()),
        "Expected one of S, MFJ, MFS, HOH, QSS but got: {}",
        code
    );
}

#[tokio::test]
async fn test_load_fails_without_tax_year_config() {
    let repo = setup_test_db_without_seeds().await;

    // Insert filing statuses but NOT tax_year_config
    sqlx::query(
        "INSERT INTO filing_status (id, status_code, status_name) VALUES
         (1, 'S', 'Single'),
         (2, 'MFJ', 'Married Filing Jointly'),
         (3, 'MFS', 'Married Filing Separately'),
         (4, 'HOH', 'Head of Household'),
         (5, 'QSS', 'Qualifying Surviving Spouse')",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert filing statuses");

    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");

    let result = TaxBracketLoader::load(&repo, &records).await;

    assert_eq!(result, Err(TaxBracketLoaderError::TaxYearNotFound(2025)));
}

#[tokio::test]
async fn test_load_preserves_other_year_brackets() {
    let repo = setup_test_db().await;

    // Insert a tax year config for 2024
    sqlx::query(
        "INSERT INTO tax_year_config (
            tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
            se_tax_deductible_percentage, se_deduction_factor, required_payment_threshold
        ) VALUES (2024, 168600, 0.062, 0.0145, 0.9235, 0.5, 1000)",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert 2024 tax year config");

    // Insert brackets for 2024
    sqlx::query(
        "INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
         VALUES (2024, 1, 0, 11000, 0.10, 0)",
    )
    .execute(repo.pool())
    .await
    .expect("Failed to insert 2024 bracket");

    // Load 2025 data
    let records = TaxBracketLoader::parse(TEST_CSV_2025.as_bytes()).expect("Failed to parse CSV");
    TaxBracketLoader::load(&repo, &records)
        .await
        .expect("Failed to load brackets");

    // 2024 brackets should still exist
    let brackets_2024 = repo
        .get_tax_brackets(2024, 1)
        .await
        .expect("Failed to get 2024 brackets");
    assert_eq!(brackets_2024.len(), 1);
    assert_eq!(brackets_2024[0].max_income, Some(dec!(11000)));

    // 2025 brackets should be loaded
    let brackets_2025 = repo
        .get_tax_brackets(2025, 1)
        .await
        .expect("Failed to get 2025 brackets");
    assert_eq!(brackets_2025.len(), 7);
}
