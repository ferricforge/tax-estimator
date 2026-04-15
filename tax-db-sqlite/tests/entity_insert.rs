use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;
use tax_core::{RepositoryError, StandardDeduction, TaxRepository};
use tax_db_sqlite::SqliteRepository;

const TEST_YEAR: i32 = 9999;
const TEST_FS_ID: i32 = 99;

/// Fresh in‑memory DB with the real schema applied. No seeds — every test
/// owns the rows it needs, so tests are isolated and order‑independent.
async fn setup_repo() -> SqliteRepository {
    let repo = SqliteRepository::new(":memory:")
        .await
        .expect("in‑memory connect");
    repo.run_migrations().await.expect("migrations");
    repo
}

/// Insert just enough FK parents for a `standard_deductions` row.
/// Uses sentinel ids that never collide with real seed data.
async fn insert_fk_parents(repo: &SqliteRepository) {
    sqlx::query(
        "INSERT INTO tax_year_config (
            tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
            se_tax_deductible_percentage, se_deduction_factor,
            required_payment_threshold, min_se_threshold
        ) VALUES (?, 200000.00, 0.125, 0.030, 0.9300, 0.55, 1500.00, 400.00)",
    )
    .bind(TEST_YEAR)
    .execute(repo.pool())
    .await
    .expect("insert tax_year_config");

    sqlx::query(
        "INSERT INTO filing_status (id, status_code, status_name)
         VALUES (?, 'S', 'Test Single')",
    )
    .bind(TEST_FS_ID)
    .execute(repo.pool())
    .await
    .expect("insert filing_status");
}

/// Full round‑trip through the `TaxRepository` trait:
///   get (NotFound) → insert → get (round‑trips) → delete → get (NotFound)
///
/// The insert path is the derive‑generated `StandardDeduction::insert`, called
/// via `SqliteRepository::insert_standard_deduction`.
#[tokio::test]
async fn standard_deduction_insert_get_delete_round_trip() {
    let repo = setup_repo().await;
    insert_fk_parents(&repo).await;

    // 1. Nothing there yet.
    let before = repo.get_standard_deduction(TEST_YEAR, TEST_FS_ID).await;
    assert!(
        matches!(before, Err(RepositoryError::NotFound)),
        "expected NotFound before insert, got {before:?}"
    );

    // 2. Insert via the derive‑generated path.
    let sd = StandardDeduction {
        tax_year: TEST_YEAR,
        filing_status_id: TEST_FS_ID,
        amount: dec!(12345.67),
    };
    repo.insert_standard_deduction(&sd)
        .await
        .expect("insert_standard_deduction");

    // 3. Read it back through the existing hand‑written get.
    let fetched = repo
        .get_standard_deduction(TEST_YEAR, TEST_FS_ID)
        .await
        .expect("row should now exist");
    assert_eq!(fetched, sd);

    // 4. Delete it.
    repo.delete_standard_deduction(TEST_YEAR, TEST_FS_ID)
        .await
        .expect("delete_standard_deduction");

    // 5. Gone again.
    let after = repo.get_standard_deduction(TEST_YEAR, TEST_FS_ID).await;
    assert!(
        matches!(after, Err(RepositoryError::NotFound)),
        "expected NotFound after delete, got {after:?}"
    );
}

#[tokio::test]
async fn delete_standard_deduction_not_found() {
    let repo = setup_repo().await;

    let result = repo.delete_standard_deduction(TEST_YEAR, TEST_FS_ID).await;

    assert!(
        matches!(result, Err(RepositoryError::NotFound)),
        "expected NotFound, got {result:?}"
    );
}

#[tokio::test]
async fn insert_standard_deduction_duplicate_pk_is_error() {
    let repo = setup_repo().await;
    insert_fk_parents(&repo).await;

    let sd = StandardDeduction {
        tax_year: TEST_YEAR,
        filing_status_id: TEST_FS_ID,
        amount: dec!(100.00),
    };

    repo.insert_standard_deduction(&sd)
        .await
        .expect("first insert succeeds");

    let dup = repo.insert_standard_deduction(&sd).await;
    assert!(
        matches!(dup, Err(RepositoryError::Database(_))),
        "second insert should violate PRIMARY KEY, got {dup:?}"
    );
}
