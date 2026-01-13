use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, sqlite::SqlitePool};
use tax_core::{
    FilingStatus, FilingStatusCode, NewTaxEstimate, RepositoryError, StandardDeduction, TaxBracket,
    TaxEstimate, TaxRepository, TaxYearConfig,
};

use crate::decimal::{decimal_to_f64, get_decimal, get_optional_decimal};

pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url)
            .await
            .with_context(|| format!("Failed to connect to database: {}", database_url))?;
        Ok(Self { pool })
    }

    pub async fn new_with_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        Ok(())
    }

    /// Load and execute all SQL seed files from the specified directory.
    /// Files are executed in alphabetical order by filename.
    pub async fn run_seeds(
        &self,
        seeds_dir: &Path,
    ) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(seeds_dir)
            .with_context(|| format!("Failed to read seeds directory '{}'", seeds_dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sql"))
            .collect();

        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let sql = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read seed file '{}'", path.display()))?;

            sqlx::raw_sql(&sql)
                .execute(&self.pool)
                .await
                .with_context(|| format!("Failed to execute seed file '{}'", path.display()))?;
        }

        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn row_to_tax_estimate(row: &sqlx::sqlite::SqliteRow) -> Result<TaxEstimate, RepositoryError> {
    Ok(TaxEstimate {
        id: row
            .try_get("id")
            .map_err(|e| RepositoryError::Database(e.to_string()))?,
        tax_year: row
            .try_get("tax_year")
            .map_err(|e| RepositoryError::Database(e.to_string()))?,
        filing_status_id: row
            .try_get("filing_status_id")
            .map_err(|e| RepositoryError::Database(e.to_string()))?,
        expected_agi: get_decimal(row, "expected_agi")?,
        expected_deduction: get_decimal(row, "expected_deduction")?,
        expected_qbi_deduction: get_optional_decimal(row, "expected_qbi_deduction")?,
        expected_amt: get_optional_decimal(row, "expected_amt")?,
        expected_credits: get_optional_decimal(row, "expected_credits")?,
        expected_other_taxes: get_optional_decimal(row, "expected_other_taxes")?,
        expected_withholding: get_optional_decimal(row, "expected_withholding")?,
        prior_year_tax: get_optional_decimal(row, "prior_year_tax")?,
        se_income: get_optional_decimal(row, "se_income")?,
        expected_crp_payments: get_optional_decimal(row, "expected_crp_payments")?,
        expected_wages: get_optional_decimal(row, "expected_wages")?,
        calculated_se_tax: get_optional_decimal(row, "calculated_se_tax")?,
        calculated_total_tax: get_optional_decimal(row, "calculated_total_tax")?,
        calculated_required_payment: get_optional_decimal(row, "calculated_required_payment")?,
        created_at: row
            .try_get::<DateTime<Utc>, _>("created_at")
            .map_err(|e| RepositoryError::Database(format!("Failed to get created_at: {}", e)))?,
        updated_at: row
            .try_get::<DateTime<Utc>, _>("updated_at")
            .map_err(|e| RepositoryError::Database(format!("Failed to get updated_at: {}", e)))?,
    })
}

#[async_trait]
impl TaxRepository for SqliteRepository {
    async fn get_tax_year_config(
        &self,
        year: i32,
    ) -> Result<TaxYearConfig, RepositoryError> {
        let row = sqlx::query(
            "SELECT tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                    se_tax_deductible_percentage, se_deduction_factor,
                    required_payment_threshold, min_se_threshold
             FROM tax_year_config WHERE tax_year = ?",
        )
        .bind(year)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        Ok(TaxYearConfig {
            tax_year: row
                .try_get("tax_year")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
            ss_wage_max: get_decimal(&row, "ss_wage_max")?,
            ss_tax_rate: get_decimal(&row, "ss_tax_rate")?,
            medicare_tax_rate: get_decimal(&row, "medicare_tax_rate")?,
            se_tax_deductible_percentage: get_decimal(&row, "se_tax_deductible_percentage")?,
            se_deduction_factor: get_decimal(&row, "se_deduction_factor")?,
            required_payment_threshold: get_decimal(&row, "required_payment_threshold")?,
            min_se_threshold: get_decimal(&row, "min_se_threshold")?,
        })
    }

    async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError> {
        let rows = sqlx::query("SELECT tax_year FROM tax_year_config ORDER BY tax_year DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.iter()
            .map(|row| {
                row.try_get("tax_year")
                    .map_err(|e| RepositoryError::Database(e.to_string()))
            })
            .collect()
    }

    async fn get_filing_status(
        &self,
        id: i32,
    ) -> Result<FilingStatus, RepositoryError> {
        let row =
            sqlx::query("SELECT id, status_code, status_name FROM filing_status WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?
                .ok_or(RepositoryError::NotFound)?;

        let status_code_str: String = row
            .try_get("status_code")
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        let status_code = FilingStatusCode::parse(&status_code_str).ok_or_else(|| {
            RepositoryError::Database(format!("Invalid status code: {}", status_code_str))
        })?;

        Ok(FilingStatus {
            id: row
                .try_get("id")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
            status_code,
            status_name: row
                .try_get("status_name")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
        })
    }

    async fn get_filing_status_by_code(
        &self,
        code: &str,
    ) -> Result<FilingStatus, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, status_code, status_name FROM filing_status WHERE status_code = ?",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        let status_code_str: String = row
            .try_get("status_code")
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        let status_code = FilingStatusCode::parse(&status_code_str).ok_or_else(|| {
            RepositoryError::Database(format!("Invalid status code: {}", status_code_str))
        })?;

        Ok(FilingStatus {
            id: row
                .try_get("id")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
            status_code,
            status_name: row
                .try_get("status_name")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
        })
    }

    async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError> {
        let rows =
            sqlx::query("SELECT id, status_code, status_name FROM filing_status ORDER BY id")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut statuses = Vec::new();
        for row in rows {
            let status_code_str: String = row
                .try_get("status_code")
                .map_err(|e| RepositoryError::Database(e.to_string()))?;
            let status_code = FilingStatusCode::parse(&status_code_str).ok_or_else(|| {
                RepositoryError::Database(format!("Invalid status code: {}", status_code_str))
            })?;

            statuses.push(FilingStatus {
                id: row
                    .try_get("id")
                    .map_err(|e| RepositoryError::Database(e.to_string()))?,
                status_code,
                status_name: row
                    .try_get("status_name")
                    .map_err(|e| RepositoryError::Database(e.to_string()))?,
            });
        }
        Ok(statuses)
    }

    async fn get_standard_deduction(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<StandardDeduction, RepositoryError> {
        let row = sqlx::query(
            "SELECT tax_year, filing_status_id, amount
             FROM standard_deductions
             WHERE tax_year = ? AND filing_status_id = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        Ok(StandardDeduction {
            tax_year: row
                .try_get("tax_year")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
            filing_status_id: row
                .try_get("filing_status_id")
                .map_err(|e| RepositoryError::Database(e.to_string()))?,
            amount: get_decimal(&row, "amount")?,
        })
    }

    async fn get_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<Vec<TaxBracket>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax
             FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ?
             ORDER BY min_income",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let mut brackets = Vec::new();
        for row in rows {
            brackets.push(TaxBracket {
                tax_year: row
                    .try_get("tax_year")
                    .map_err(|e| RepositoryError::Database(e.to_string()))?,
                filing_status_id: row
                    .try_get("filing_status_id")
                    .map_err(|e| RepositoryError::Database(e.to_string()))?,
                min_income: get_decimal(&row, "min_income")?,
                max_income: get_optional_decimal(&row, "max_income")?,
                tax_rate: get_decimal(&row, "tax_rate")?,
                base_tax: get_decimal(&row, "base_tax")?,
            });
        }
        Ok(brackets)
    }

    async fn insert_tax_bracket(
        &self,
        bracket: &TaxBracket,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bracket.tax_year)
        .bind(bracket.filing_status_id)
        .bind(decimal_to_f64(bracket.min_income))
        .bind(bracket.max_income.map(decimal_to_f64))
        .bind(decimal_to_f64(bracket.tax_rate))
        .bind(decimal_to_f64(bracket.base_tax))
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn delete_tax_brackets(
        &self,
        tax_year: i32,
        filing_status_id: i32,
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM tax_brackets WHERE tax_year = ? AND filing_status_id = ?")
            .bind(tax_year)
            .bind(filing_status_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    async fn create_estimate(
        &self,
        estimate: NewTaxEstimate,
    ) -> Result<TaxEstimate, RepositoryError> {
        let now = Utc::now();

        let result = sqlx::query(
            "INSERT INTO tax_estimate (
                tax_year, filing_status_id, expected_agi, expected_deduction,
                expected_qbi_deduction, expected_amt, expected_credits,
                expected_other_taxes, expected_withholding, prior_year_tax,
                se_income, expected_crp_payments, expected_wages,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(estimate.tax_year)
        .bind(estimate.filing_status_id)
        .bind(decimal_to_f64(estimate.expected_agi))
        .bind(decimal_to_f64(estimate.expected_deduction))
        .bind(estimate.expected_qbi_deduction.map(decimal_to_f64))
        .bind(estimate.expected_amt.map(decimal_to_f64))
        .bind(estimate.expected_credits.map(decimal_to_f64))
        .bind(estimate.expected_other_taxes.map(decimal_to_f64))
        .bind(estimate.expected_withholding.map(decimal_to_f64))
        .bind(estimate.prior_year_tax.map(decimal_to_f64))
        .bind(estimate.se_income.map(decimal_to_f64))
        .bind(estimate.expected_crp_payments.map(decimal_to_f64))
        .bind(estimate.expected_wages.map(decimal_to_f64))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        let id = result.last_insert_rowid();
        self.get_estimate(id).await
    }

    async fn get_estimate(
        &self,
        id: i64,
    ) -> Result<TaxEstimate, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, tax_year, filing_status_id, expected_agi, expected_deduction,
                    expected_qbi_deduction, expected_amt, expected_credits,
                    expected_other_taxes, expected_withholding, prior_year_tax,
                    se_income, expected_crp_payments, expected_wages,
                    calculated_se_tax, calculated_total_tax, calculated_required_payment,
                    created_at, updated_at
             FROM tax_estimate WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?
        .ok_or(RepositoryError::NotFound)?;

        row_to_tax_estimate(&row)
    }

    async fn update_estimate(
        &self,
        estimate: &TaxEstimate,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now();

        let result = sqlx::query(
            "UPDATE tax_estimate SET
                tax_year = ?, filing_status_id = ?, expected_agi = ?, expected_deduction = ?,
                expected_qbi_deduction = ?, expected_amt = ?, expected_credits = ?,
                expected_other_taxes = ?, expected_withholding = ?, prior_year_tax = ?,
                se_income = ?, expected_crp_payments = ?, expected_wages = ?,
                calculated_se_tax = ?, calculated_total_tax = ?, calculated_required_payment = ?,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(estimate.tax_year)
        .bind(estimate.filing_status_id)
        .bind(decimal_to_f64(estimate.expected_agi))
        .bind(decimal_to_f64(estimate.expected_deduction))
        .bind(estimate.expected_qbi_deduction.map(decimal_to_f64))
        .bind(estimate.expected_amt.map(decimal_to_f64))
        .bind(estimate.expected_credits.map(decimal_to_f64))
        .bind(estimate.expected_other_taxes.map(decimal_to_f64))
        .bind(estimate.expected_withholding.map(decimal_to_f64))
        .bind(estimate.prior_year_tax.map(decimal_to_f64))
        .bind(estimate.se_income.map(decimal_to_f64))
        .bind(estimate.expected_crp_payments.map(decimal_to_f64))
        .bind(estimate.expected_wages.map(decimal_to_f64))
        .bind(estimate.calculated_se_tax.map(decimal_to_f64))
        .bind(estimate.calculated_total_tax.map(decimal_to_f64))
        .bind(estimate.calculated_required_payment.map(decimal_to_f64))
        .bind(now)
        .bind(estimate.id)
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn delete_estimate(
        &self,
        id: i64,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM tax_estimate WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn list_estimates(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<TaxEstimate>, RepositoryError> {
        const BASE_QUERY: &str =
            "SELECT id, tax_year, filing_status_id, expected_agi, expected_deduction,
                expected_qbi_deduction, expected_amt, expected_credits,
                expected_other_taxes, expected_withholding, prior_year_tax,
                se_income, expected_crp_payments, expected_wages,
                calculated_se_tax, calculated_total_tax, calculated_required_payment,
                created_at, updated_at
         FROM tax_estimate";

        let rows = match tax_year {
            Some(year) => {
                sqlx::query(&format!(
                    "{} WHERE tax_year = ? ORDER BY updated_at DESC",
                    BASE_QUERY
                ))
                .bind(year)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(&format!("{} ORDER BY updated_at DESC", BASE_QUERY))
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        rows.iter().map(row_to_tax_estimate).collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

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

    async fn insert_test_tax_year_config(repo: &SqliteRepository) {
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES (9999, 200000.00, 0.125, 0.030, 0.9300, 0.55, 1500.00, 400.00)",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test tax year config");
    }

    async fn setup_clean_filing_status(repo: &SqliteRepository) {
        // Clear all dependent data first, then filing statuses
        sqlx::query("DELETE FROM tax_estimate")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax estimates");
        sqlx::query("DELETE FROM standard_deductions")
            .execute(repo.pool())
            .await
            .expect("Failed to clear standard deductions");
        sqlx::query("DELETE FROM tax_brackets")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax brackets");
        sqlx::query("DELETE FROM filing_status")
            .execute(repo.pool())
            .await
            .expect("Failed to clear filing statuses");

        // Insert test-specific filing status
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (99, 'S', 'Test Single')",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test filing status");
    }

    async fn insert_test_standard_deduction(repo: &SqliteRepository) {
        insert_test_tax_year_config(repo).await;
        setup_clean_filing_status(repo).await;

        sqlx::query(
            "INSERT INTO standard_deductions (tax_year, filing_status_id, amount)
             VALUES (9999, 99, 18000.00)",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test standard deduction");
    }

    async fn insert_test_tax_brackets(repo: &SqliteRepository) {
        insert_test_tax_year_config(repo).await;
        setup_clean_filing_status(repo).await;

        sqlx::query(
            "INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES
             (9999, 99, 0, 10000, 0.10, 0),
             (9999, 99, 10000, 50000, 0.15, 1000),
             (9999, 99, 50000, NULL, 0.25, 7000)",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test tax brackets");
    }

    async fn setup_test_data_for_estimates(repo: &SqliteRepository) {
        // Clear existing data and insert test-specific data
        sqlx::query("DELETE FROM tax_estimate")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax estimates");
        sqlx::query("DELETE FROM standard_deductions")
            .execute(repo.pool())
            .await
            .expect("Failed to clear standard deductions");
        sqlx::query("DELETE FROM tax_brackets")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax brackets");
        sqlx::query("DELETE FROM filing_status")
            .execute(repo.pool())
            .await
            .expect("Failed to clear filing statuses");
        sqlx::query("DELETE FROM tax_year_config")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax year config");

        // Insert test tax years
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES
            (8888, 180000.00, 0.124, 0.029, 0.9235, 0.50, 1000.00, 400.00),
            (8887, 175000.00, 0.124, 0.029, 0.9235, 0.50, 1000.00, 400.00)",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test tax years");

        // Insert test filing status
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (50, 'S', 'Test Single')",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test filing status");
    }

    fn create_test_estimate() -> NewTaxEstimate {
        NewTaxEstimate {
            tax_year: 8888,
            filing_status_id: 50,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: Some(dec!(5000.00)),
            expected_amt: None,
            expected_credits: Some(dec!(2000.00)),
            expected_other_taxes: None,
            expected_withholding: Some(dec!(8000.00)),
            prior_year_tax: Some(dec!(12000.00)),
            se_income: Some(dec!(50000.00)),
            expected_crp_payments: None,
            expected_wages: Some(dec!(50000.00)),
        }
    }

    fn create_minimal_test_estimate() -> NewTaxEstimate {
        NewTaxEstimate {
            tax_year: 8888,
            filing_status_id: 50,
            expected_agi: dec!(75000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        }
    }

    #[tokio::test]
    async fn test_get_tax_year_config() {
        let repo = setup_test_db().await;
        insert_test_tax_year_config(&repo).await;

        let config = repo
            .get_tax_year_config(9999)
            .await
            .expect("Should find test config");

        assert_eq!(config.tax_year, 9999);
        assert_eq!(config.ss_wage_max, dec!(200000.00));
        assert_eq!(config.ss_tax_rate, dec!(0.125));
        assert_eq!(config.medicare_tax_rate, dec!(0.030));
        assert_eq!(config.se_tax_deductible_percentage, dec!(0.9300));
        assert_eq!(config.se_deduction_factor, dec!(0.55));
        assert_eq!(config.required_payment_threshold, dec!(1500.00));
        assert_eq!(config.min_se_threshold, dec!(400.00));
    }

    #[tokio::test]
    async fn test_get_tax_year_config_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_tax_year_config(1999).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_list_tax_years() {
        let repo = setup_test_db().await;
        insert_test_tax_year_config(&repo).await;

        let years = repo.list_tax_years().await.expect("Should list tax years");

        assert!(years.contains(&9999));
    }

    async fn clear_all_data(repo: &SqliteRepository) {
        // Clear all tables in dependency order
        sqlx::query("DELETE FROM tax_estimate")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax estimates");
        sqlx::query("DELETE FROM standard_deductions")
            .execute(repo.pool())
            .await
            .expect("Failed to clear standard deductions");
        sqlx::query("DELETE FROM tax_brackets")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax brackets");
        sqlx::query("DELETE FROM filing_status")
            .execute(repo.pool())
            .await
            .expect("Failed to clear filing statuses");
        sqlx::query("DELETE FROM tax_year_config")
            .execute(repo.pool())
            .await
            .expect("Failed to clear tax year config");
    }

    #[tokio::test]
    async fn test_list_filing_statuses() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;

        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name) VALUES
             (10, 'S', 'Test Single'),
             (20, 'MFJ', 'Test Married Filing Jointly')",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test filing statuses");

        let statuses = repo
            .list_filing_statuses()
            .await
            .expect("Should list filing statuses");

        assert_eq!(statuses.len(), 2);

        let single_status = statuses.iter().find(|s| s.id == 10).unwrap();
        assert_eq!(single_status.status_code, FilingStatusCode::Single);
        assert_eq!(single_status.status_name, "Test Single");

        let mfj_status = statuses.iter().find(|s| s.id == 20).unwrap();
        assert_eq!(
            mfj_status.status_code,
            FilingStatusCode::MarriedFilingJointly
        );
        assert_eq!(mfj_status.status_name, "Test Married Filing Jointly");
    }

    #[tokio::test]
    async fn test_get_filing_status() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;

        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (42, 'HOH', 'Test Head of Household')",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test filing status");

        let status = repo
            .get_filing_status(42)
            .await
            .expect("Should find test filing status");

        assert_eq!(status.id, 42);
        assert_eq!(status.status_code, FilingStatusCode::HeadOfHousehold);
        assert_eq!(status.status_name, "Test Head of Household");
    }

    #[tokio::test]
    async fn test_get_filing_status_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_filing_status(999).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_get_standard_deduction() {
        let repo = setup_test_db().await;
        insert_test_standard_deduction(&repo).await;

        let deduction = repo
            .get_standard_deduction(9999, 99)
            .await
            .expect("Should find test standard deduction");

        assert_eq!(deduction.tax_year, 9999);
        assert_eq!(deduction.filing_status_id, 99);
        assert_eq!(deduction.amount, dec!(18000.00));
    }

    #[tokio::test]
    async fn test_get_standard_deduction_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_standard_deduction(1999, 1).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_get_tax_brackets() {
        let repo = setup_test_db().await;
        insert_test_tax_brackets(&repo).await;

        let brackets = repo
            .get_tax_brackets(9999, 99)
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
        assert!(brackets[2].max_income.is_none());
        assert_eq!(brackets[2].tax_rate, dec!(0.25));
        assert_eq!(brackets[2].base_tax, dec!(7000));
    }

    #[tokio::test]
    async fn test_get_tax_brackets_empty() {
        let repo = setup_test_db().await;

        let brackets = repo
            .get_tax_brackets(1999, 1)
            .await
            .expect("Should return empty vec");

        assert!(brackets.is_empty());
    }

    #[tokio::test]
    async fn test_insert_tax_bracket() {
        let repo = setup_test_db().await;
        insert_test_tax_year_config(&repo).await;
        setup_clean_filing_status(&repo).await;

        let bracket = TaxBracket {
            tax_year: 9999,
            filing_status_id: 99,
            min_income: dec!(0),
            max_income: Some(dec!(20000)),
            tax_rate: dec!(0.12),
            base_tax: dec!(0),
        };

        repo.insert_tax_bracket(&bracket)
            .await
            .expect("Should insert bracket");

        let brackets = repo
            .get_tax_brackets(9999, 99)
            .await
            .expect("Should get brackets");

        assert_eq!(brackets.len(), 1);
        assert_eq!(brackets[0].min_income, dec!(0));
        assert_eq!(brackets[0].max_income, Some(dec!(20000)));
        assert_eq!(brackets[0].tax_rate, dec!(0.12));
        assert_eq!(brackets[0].base_tax, dec!(0));
    }

    #[tokio::test]
    async fn test_insert_tax_bracket_with_null_max() {
        let repo = setup_test_db().await;
        insert_test_tax_year_config(&repo).await;
        setup_clean_filing_status(&repo).await;

        let bracket = TaxBracket {
            tax_year: 9999,
            filing_status_id: 99,
            min_income: dec!(100000),
            max_income: None,
            tax_rate: dec!(0.37),
            base_tax: dec!(25000),
        };

        repo.insert_tax_bracket(&bracket)
            .await
            .expect("Should insert bracket");

        let brackets = repo
            .get_tax_brackets(9999, 99)
            .await
            .expect("Should get brackets");

        assert_eq!(brackets.len(), 1);
        assert_eq!(brackets[0].max_income, None);
    }

    #[tokio::test]
    async fn test_delete_tax_brackets() {
        let repo = setup_test_db().await;
        insert_test_tax_brackets(&repo).await;

        let brackets_before = repo
            .get_tax_brackets(9999, 99)
            .await
            .expect("Should get brackets");
        assert_eq!(brackets_before.len(), 3);

        repo.delete_tax_brackets(9999, 99)
            .await
            .expect("Should delete brackets");

        let brackets_after = repo
            .get_tax_brackets(9999, 99)
            .await
            .expect("Should get brackets");
        assert!(brackets_after.is_empty());
    }

    #[tokio::test]
    async fn test_delete_tax_brackets_nonexistent() {
        let repo = setup_test_db().await;

        // Should not error when deleting nonexistent brackets
        repo.delete_tax_brackets(9999, 99)
            .await
            .expect("Should succeed even if no brackets exist");
    }

    #[tokio::test]
    async fn test_get_filing_status_by_code() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;

        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (7, 'MFJ', 'Test Married Filing Jointly')",
        )
        .execute(repo.pool())
        .await
        .expect("Failed to insert test filing status");

        let status = repo
            .get_filing_status_by_code("MFJ")
            .await
            .expect("Should find filing status by code");

        assert_eq!(status.id, 7);
        assert_eq!(status.status_code, FilingStatusCode::MarriedFilingJointly);
        assert_eq!(status.status_name, "Test Married Filing Jointly");
    }

    #[tokio::test]
    async fn test_get_filing_status_by_code_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_filing_status_by_code("INVALID").await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_create_and_get_estimate() {
        let repo = setup_test_db().await;
        setup_test_data_for_estimates(&repo).await;

        let new_estimate = create_test_estimate();
        let created = repo
            .create_estimate(new_estimate)
            .await
            .expect("Should create estimate");

        assert!(created.id > 0);
        assert_eq!(created.tax_year, 8888);
        assert_eq!(created.filing_status_id, 50);
        assert_eq!(created.expected_agi, dec!(100000.00));
        assert_eq!(created.expected_deduction, dec!(15000.00));
        assert_eq!(created.expected_qbi_deduction, Some(dec!(5000.00)));
        assert_eq!(created.expected_amt, None);
        assert_eq!(created.expected_credits, Some(dec!(2000.00)));
        assert_eq!(created.expected_other_taxes, None);
        assert_eq!(created.expected_withholding, Some(dec!(8000.00)));
        assert_eq!(created.prior_year_tax, Some(dec!(12000.00)));
        assert_eq!(created.se_income, Some(dec!(50000.00)));
        assert_eq!(created.expected_crp_payments, None);
        assert_eq!(created.expected_wages, Some(dec!(50000.00)));
        assert_eq!(created.calculated_se_tax, None);
        assert_eq!(created.calculated_total_tax, None);
        assert_eq!(created.calculated_required_payment, None);

        let fetched = repo
            .get_estimate(created.id)
            .await
            .expect("Should fetch estimate");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.expected_agi, dec!(100000.00));
    }

    #[tokio::test]
    async fn test_get_estimate_not_found() {
        let repo = setup_test_db().await;

        let result = repo.get_estimate(99999).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_update_estimate() {
        let repo = setup_test_db().await;
        setup_test_data_for_estimates(&repo).await;

        let new_estimate = create_minimal_test_estimate();
        let mut created = repo
            .create_estimate(new_estimate)
            .await
            .expect("Should create estimate");

        created.expected_agi = dec!(150000.00);
        created.calculated_total_tax = Some(dec!(25000.00));
        created.calculated_se_tax = Some(dec!(7500.00));
        created.calculated_required_payment = Some(dec!(4000.00));

        repo.update_estimate(&created)
            .await
            .expect("Should update estimate");

        let fetched = repo
            .get_estimate(created.id)
            .await
            .expect("Should fetch estimate");

        assert_eq!(fetched.expected_agi, dec!(150000.00));
        assert_eq!(fetched.calculated_total_tax, Some(dec!(25000.00)));
        assert_eq!(fetched.calculated_se_tax, Some(dec!(7500.00)));
        assert_eq!(fetched.calculated_required_payment, Some(dec!(4000.00)));
    }

    #[tokio::test]
    async fn test_update_estimate_not_found() {
        let repo = setup_test_db().await;
        setup_test_data_for_estimates(&repo).await;

        let new_estimate = create_minimal_test_estimate();
        let mut created = repo
            .create_estimate(new_estimate)
            .await
            .expect("Should create estimate");

        created.id = 99999;

        let result = repo.update_estimate(&created).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_estimate() {
        let repo = setup_test_db().await;
        setup_test_data_for_estimates(&repo).await;

        let new_estimate = create_minimal_test_estimate();
        let created = repo
            .create_estimate(new_estimate)
            .await
            .expect("Should create estimate");
        let id = created.id;

        repo.delete_estimate(id)
            .await
            .expect("Should delete estimate");

        let result = repo.get_estimate(id).await;
        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_delete_estimate_not_found() {
        let repo = setup_test_db().await;

        let result = repo.delete_estimate(99999).await;

        assert_eq!(result, Err(RepositoryError::NotFound));
    }

    #[tokio::test]
    async fn test_list_estimates() {
        let repo = setup_test_db().await;
        setup_test_data_for_estimates(&repo).await;

        let estimate_8888 = NewTaxEstimate {
            tax_year: 8888,
            filing_status_id: 50,
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        };

        let estimate_8887 = NewTaxEstimate {
            tax_year: 8887,
            filing_status_id: 50,
            expected_agi: dec!(90000.00),
            expected_deduction: dec!(14000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
        };

        repo.create_estimate(estimate_8888.clone())
            .await
            .expect("Should create estimate");
        repo.create_estimate(estimate_8888)
            .await
            .expect("Should create estimate");
        repo.create_estimate(estimate_8887)
            .await
            .expect("Should create estimate");

        let all = repo
            .list_estimates(None)
            .await
            .expect("Should list all estimates");
        assert_eq!(all.len(), 3);

        let for_8888 = repo
            .list_estimates(Some(8888))
            .await
            .expect("Should list for 8888");
        assert_eq!(for_8888.len(), 2);
        assert!(for_8888.iter().all(|e| e.tax_year == 8888));

        let for_8887 = repo
            .list_estimates(Some(8887))
            .await
            .expect("Should list for 8887");
        assert_eq!(for_8887.len(), 1);
        assert_eq!(for_8887[0].tax_year, 8887);

        let for_7777 = repo
            .list_estimates(Some(7777))
            .await
            .expect("Should list for 7777");
        assert!(for_7777.is_empty());
    }

    #[tokio::test]
    async fn test_run_seeds() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;

        let seeds_dir = std::path::Path::new("./seeds");
        repo.run_seeds(seeds_dir)
            .await
            .expect("Should run seeds successfully");

        // Verify filing statuses were seeded
        let statuses = repo
            .list_filing_statuses()
            .await
            .expect("Should list filing statuses");
        assert_eq!(statuses.len(), 5);

        // Verify tax year config was seeded
        let config = repo
            .get_tax_year_config(2025)
            .await
            .expect("Should find 2025 config");
        assert_eq!(config.tax_year, 2025);

        // Verify standard deductions were seeded
        let deduction = repo
            .get_standard_deduction(2025, 1)
            .await
            .expect("Should find standard deduction");
        assert_eq!(deduction.tax_year, 2025);
        assert_eq!(deduction.filing_status_id, 1);

        // Verify tax brackets were seeded
        let brackets = repo
            .get_tax_brackets(2025, 1)
            .await
            .expect("Should find tax brackets");
        assert_eq!(brackets.len(), 7);
    }

    #[tokio::test]
    async fn test_run_seeds_nonexistent_directory() {
        let repo = setup_test_db().await;

        let result = repo.run_seeds(std::path::Path::new("./nonexistent")).await;

        let err = result.expect_err("Should fail for nonexistent directory");
        assert_eq!(
            err.to_string(),
            "Failed to read seeds directory './nonexistent'"
        );
    }
}
