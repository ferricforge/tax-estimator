//! Demonstrates that a SQLite `RETURNING` expression can read a related table
//! through a correlated scalar subquery, even though `RETURNING` has no
//! top-level `FROM` or `JOIN` clause.

use tax_db_sqlite::SqliteRepository;

#[tokio::test]
async fn returning_scalar_subquery_reads_filing_status() {
    let repo = SqliteRepository::new(":memory:")
        .await
        .expect("open repository");
    repo.run_migrations().await.expect("run migrations");

    sqlx::query(
        "INSERT INTO filing_status (id, status_code, status_name)
         VALUES (1, 'S', 'Single')",
    )
    .execute(repo.pool())
    .await
    .expect("insert filing status");
    sqlx::query(
        "INSERT INTO tax_year_config (
             tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
             se_tax_deductible_percentage, se_deduction_factor,
             required_payment_threshold, min_se_threshold
         ) VALUES (2025, 176100, 0.062, 0.029, 0.5, 0.9235, 1000, 400)",
    )
    .execute(repo.pool())
    .await
    .expect("insert tax-year configuration");

    let filing_status_code: String = sqlx::query_scalar(
        "INSERT INTO tax_estimate (
             tax_year, filing_status_id, expected_agi, expected_deduction
         ) VALUES (?, ?, ?, ?)
         ON CONFLICT (tax_year, filing_status_id) DO UPDATE SET
             expected_agi = excluded.expected_agi,
             expected_deduction = excluded.expected_deduction,
             updated_at = CURRENT_TIMESTAMP
         RETURNING (
             SELECT fs.status_code
             FROM filing_status AS fs
             WHERE fs.id = tax_estimate.filing_status_id
         ) AS filing_status_code",
    )
    .bind(2025)
    .bind(1)
    .bind(100_000)
    .bind(15_000)
    .fetch_one(repo.pool())
    .await
    .expect("insert estimate and return filing-status code");

    println!("RETURNING scalar subquery resolved filing_status.status_code = {filing_status_code}");
    assert_eq!(filing_status_code, "S");
}
