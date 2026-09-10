//! Integration coverage for [`SqliteRepository::backup_to`].
//!
//! These tests exercise the method's documented contract end to end: a backup
//! must be a single, self-contained, structurally valid database file that
//! carries every committed estimate (input *and* computed values) plus all
//! reference data, even when the committed data still lives in the source WAL
//! and even after the source database is gone. A separate test covers the
//! documented requirement that the destination must not already exist.

use std::path::{Path, PathBuf};

use pretty_assertions::assert_eq;
use rust_decimal_macros::dec;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use tax_core::{
    FilingStatusCode, TaxEstimate, TaxEstimateComputed, TaxEstimateInput, TaxRepository,
};
use tax_db_sqlite::SqliteRepository;
use tempfile::tempdir;

/// The `-wal` and `-shm` sidecar paths SQLite derives from a database file path.
fn sidecar_paths(database: &Path) -> (PathBuf, PathBuf) {
    let mut wal = database.as_os_str().to_owned();
    wal.push("-wal");
    let mut shm = database.as_os_str().to_owned();
    shm.push("-shm");
    (PathBuf::from(wal), PathBuf::from(shm))
}

/// Size of the database's `-wal` sidecar, treating a missing sidecar as empty.
fn wal_len(database: &Path) -> u64 {
    let (wal, _) = sidecar_paths(database);
    std::fs::metadata(wal).map(|meta| meta.len()).unwrap_or(0)
}

/// Open a file-backed repository whose source database is kept in WAL mode on a
/// single connection, so committed writes stay resident in the `-wal` sidecar
/// until something explicitly checkpoints it.
async fn open_wal_source(path: &Path) -> SqliteRepository {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open source database in WAL mode");
    SqliteRepository::new_with_pool(pool).await
}

/// An estimate input with every optional field populated.
fn fully_populated_input() -> TaxEstimateInput {
    TaxEstimateInput {
        tax_year: 2025,
        filing_status: FilingStatusCode::Single,
        se_income: Some(dec!(50000.00)),
        expected_crp_payments: Some(dec!(1200.00)),
        expected_wages: Some(dec!(40000.00)),
        expected_agi: dec!(100000.00),
        expected_deduction: dec!(15000.00),
        expected_qbi_deduction: Some(dec!(5000.00)),
        expected_amt: Some(dec!(750.00)),
        expected_credits: Some(dec!(2000.00)),
        expected_other_taxes: Some(dec!(300.00)),
        expected_withholding: Some(dec!(8000.00)),
        prior_year_tax: Some(dec!(12000.00)),
    }
}

fn assert_estimate_matches(
    actual: &TaxEstimate,
    expected_input: &TaxEstimateInput,
    expected_computed: &TaxEstimateComputed,
) {
    assert_eq!(actual.input.tax_year, expected_input.tax_year);
    assert_eq!(actual.input.filing_status, expected_input.filing_status);
    assert_eq!(actual.input.se_income, expected_input.se_income);
    assert_eq!(
        actual.input.expected_crp_payments,
        expected_input.expected_crp_payments
    );
    assert_eq!(actual.input.expected_wages, expected_input.expected_wages);
    assert_eq!(actual.input.expected_agi, expected_input.expected_agi);
    assert_eq!(
        actual.input.expected_deduction,
        expected_input.expected_deduction
    );
    assert_eq!(
        actual.input.expected_qbi_deduction,
        expected_input.expected_qbi_deduction
    );
    assert_eq!(actual.input.expected_amt, expected_input.expected_amt);
    assert_eq!(
        actual.input.expected_credits,
        expected_input.expected_credits
    );
    assert_eq!(
        actual.input.expected_other_taxes,
        expected_input.expected_other_taxes
    );
    assert_eq!(
        actual.input.expected_withholding,
        expected_input.expected_withholding
    );
    assert_eq!(actual.input.prior_year_tax, expected_input.prior_year_tax);
    assert_eq!(actual.computed.as_ref(), Some(expected_computed));
}

#[tokio::test]
async fn backup_to_produces_a_populated_standalone_copy() {
    let workspace = tempdir().expect("create temp workspace");
    let source = workspace.path().join("source.db");
    let dest = workspace.path().join("backup.db");

    // A file-backed WAL source is the scenario `VACUUM INTO` exists to handle.
    let repo = open_wal_source(&source).await;
    repo.run_migrations().await.expect("run migrations");
    repo.run_seeds(Path::new("./seeds"))
        .await
        .expect("seed reference data");

    // Migrations and seeds have already written to the WAL. Stop automatic
    // checkpointing and truncate the WAL so that anything it contains afterward
    // can only have come from the estimate writes below.
    sqlx::query("PRAGMA wal_autocheckpoint = 0")
        .execute(repo.pool())
        .await
        .expect("disable automatic WAL checkpointing");
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(repo.pool())
        .await
        .expect("truncate the WAL");
    assert_eq!(
        wal_len(&source),
        0,
        "the WAL should be empty after truncating it"
    );

    // A committed estimate with every input field set and every computed field set.
    let mut estimate = repo
        .create_estimate(fully_populated_input())
        .await
        .expect("create estimate");
    let estimate_id = estimate.id;
    estimate.computed = Some(TaxEstimateComputed {
        se_tax: dec!(7050.00),
        total_tax: dec!(18250.00),
        required_payment: dec!(4125.00),
    });
    repo.update_estimate(&estimate)
        .await
        .expect("store computed values");

    let expected_input = fully_populated_input();
    let expected_computed = TaxEstimateComputed {
        se_tax: dec!(7050.00),
        total_tax: dec!(18250.00),
        required_payment: dec!(4125.00),
    };

    // The estimate is committed but never checkpointed, so it still lives in the
    // source WAL: `backup_to` must read through the WAL to capture it.
    assert!(
        wal_len(&source) > 0,
        "the committed estimate should now be resident in the source WAL"
    );

    repo.backup_to(&dest).await.expect("write the backup");

    // The backup is a single file: no `-wal`/`-shm` sidecars, regardless of the
    // source journal mode.
    let (dest_wal, dest_shm) = sidecar_paths(&dest);
    assert!(dest.exists(), "backup file should exist");
    assert!(!dest_wal.exists(), "backup must not have a -wal sidecar");
    assert!(!dest_shm.exists(), "backup must not have a -shm sidecar");

    // The source can now go away entirely.
    let (source_wal, source_shm) = sidecar_paths(&source);
    repo.pool().close().await;
    drop(repo);
    std::fs::remove_file(&source).expect("remove source database");
    let _ = std::fs::remove_file(&source_wal);
    let _ = std::fs::remove_file(&source_shm);

    // The copy opens on its own, with the source gone.
    let copy = SqliteRepository::new(dest.to_str().expect("UTF-8 backup path"))
        .await
        .expect("open the backup after the source is removed");

    // It carries the full committed estimate: complete input and computed values.
    let listed = copy
        .list_estimates(None)
        .await
        .expect("list estimates in backup");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, estimate_id);
    assert_estimate_matches(&listed[0], &expected_input, &expected_computed);

    let fetched = copy
        .get_estimate(estimate_id)
        .await
        .expect("read estimate in backup");
    assert_estimate_matches(&fetched, &expected_input, &expected_computed);

    // Required reference data was copied in full.
    let statuses = copy
        .list_filing_statuses()
        .await
        .expect("reference filing statuses");
    assert_eq!(statuses.len(), 5);

    let config = copy
        .get_tax_year_config(2025)
        .await
        .expect("reference tax year config");
    assert_eq!(config.tax_year, 2025);

    let filing_status_data = copy
        .get_filing_status_data(2025)
        .await
        .expect("reference deductions and brackets");
    assert_eq!(filing_status_data.len(), 5);

    let (single_status, single_deduction, single_brackets) = filing_status_data
        .iter()
        .find(|(fs, _, _)| fs.status_code == FilingStatusCode::Single)
        .expect("single filing status present in backup");
    assert_eq!(single_deduction.tax_year, 2025);
    assert_eq!(single_deduction.filing_status_id, single_status.id);
    assert_eq!(
        single_brackets.len(),
        7,
        "single's 2025 tax brackets should all be copied"
    );

    let deduction = copy
        .get_standard_deduction(2025, single_status.id)
        .await
        .expect("reference standard deduction");
    assert_eq!(deduction.tax_year, 2025);
    assert_eq!(deduction.filing_status_id, single_status.id);
    assert_eq!(deduction.amount, single_deduction.amount);

    // The backup is structurally valid.
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(copy.pool())
        .await
        .expect("run integrity_check on the backup");
    assert_eq!(integrity, "ok");

    let foreign_key_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(copy.pool())
        .await
        .expect("run foreign_key_check on the backup");
    assert!(
        foreign_key_violations.is_empty(),
        "backup should have no dangling foreign keys"
    );

    copy.pool().close().await;
}

#[tokio::test]
async fn backup_to_refuses_an_existing_destination() {
    let workspace = tempdir().expect("create temp workspace");
    let source = workspace.path().join("source.db");
    let dest = workspace.path().join("already-here.db");

    let repo = open_wal_source(&source).await;
    repo.run_migrations().await.expect("run migrations");

    // The documented contract: `dest` must not already exist. Stand up a real,
    // populated SQLite database there.
    {
        let existing = SqliteRepository::new(dest.to_str().expect("UTF-8 destination path"))
            .await
            .expect("open pre-existing destination");
        existing.run_migrations().await.expect("run migrations");
        existing
            .run_seeds(Path::new("./seeds"))
            .await
            .expect("seed pre-existing destination");
        existing
            .create_estimate(fully_populated_input())
            .await
            .expect("populate pre-existing destination");
        existing
            .checkpoint()
            .await
            .expect("flush pre-existing destination");
        existing.pool().close().await;
    }

    let err = repo
        .backup_to(&dest)
        .await
        .expect_err("backup_to must not overwrite an existing destination");

    // Assert against the stable context `backup_to` adds, not SQLite's own
    // wording, which varies by version.
    let report = format!("{err:#}");
    let expected_context = format!("Failed to copy the database to '{}'", dest.display());
    assert!(
        report.contains(&expected_context),
        "expected the backup_to failure context, got: {report}"
    );

    // The pre-existing database is left intact.
    let untouched = SqliteRepository::new(dest.to_str().expect("UTF-8 destination path"))
        .await
        .expect("reopen pre-existing destination");
    let estimates = untouched
        .list_estimates(None)
        .await
        .expect("pre-existing destination still readable");
    assert_eq!(estimates.len(), 1);
    assert_eq!(estimates[0].input, fully_populated_input());
    untouched.pool().close().await;

    repo.pool().close().await;
}

#[tokio::test]
async fn backup_to_accepts_a_variety_of_destination_path_characters() {
    let workspace = tempdir().expect("create temp workspace");
    let source = workspace.path().join("source.db");

    let repo = open_wal_source(&source).await;
    repo.run_migrations().await.expect("run migrations");
    repo.run_seeds(Path::new("./seeds"))
        .await
        .expect("seed reference data");
    let created = repo
        .create_estimate(fully_populated_input())
        .await
        .expect("create estimate");

    // Apostrophe (must survive `VACUUM INTO`'s `''` escaping), spaces,
    // diacritics, accent marks, and non-Latin scripts.
    let destination_names = [
        "l'année fiscale.db",
        "sauvegarde café résumé.db",
        "Łódź_żółć.db",
        "δοκιμή_税金データ.db",
    ];

    for name in destination_names {
        let dest = workspace.path().join(name);

        repo.backup_to(&dest)
            .await
            .unwrap_or_else(|err| panic!("backup_to should accept {name:?}: {err:#}"));

        let (dest_wal, dest_shm) = sidecar_paths(&dest);
        assert!(dest.exists(), "backup {name:?} should exist");
        assert!(!dest_wal.exists(), "backup {name:?} must not have a -wal sidecar");
        assert!(!dest_shm.exists(), "backup {name:?} must not have a -shm sidecar");

        let copy = SqliteRepository::new(dest.to_str().expect("destination path is UTF-8"))
            .await
            .unwrap_or_else(|err| panic!("should open backup {name:?}: {err:#}"));
        let estimates = copy
            .list_estimates(None)
            .await
            .unwrap_or_else(|err| panic!("should list estimates from {name:?}: {err:#}"));
        assert_eq!(estimates.len(), 1, "backup {name:?} should carry the estimate");
        assert_eq!(estimates[0].id, created.id);
        copy.pool().close().await;
    }

    repo.pool().close().await;
}

#[tokio::test]
async fn backup_to_reports_an_error_for_an_invalid_destination_path() {
    let workspace = tempdir().expect("create temp workspace");
    let source = workspace.path().join("source.db");
    let base = workspace.path().to_str().expect("temp workspace path is UTF-8");

    let repo = open_wal_source(&source).await;
    repo.run_migrations().await.expect("run migrations");

    // An embedded NUL byte is never a valid path component. `backup_to` must
    // surface this through its normal error path, not panic.
    let invalid = format!("{base}/inva\u{0}lid.db");
    let result = repo.backup_to(Path::new(&invalid)).await;

    let err = result.expect_err("backup_to must reject a destination path with a NUL byte");
    assert!(
        format!("{err:#}").contains("Failed to copy the database to"),
        "expected the backup_to failure context, got: {err:#}"
    );

    repo.pool().close().await;
}
