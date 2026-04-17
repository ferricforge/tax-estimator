use async_trait::async_trait;
use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{Persist, RepositoryError, StandardDeduction};

use crate::SqliteRepository;
use crate::decimal::{decimal_to_f64, get_decimal};
use crate::repository::db_err;

fn from_row(row: &SqliteRow) -> Result<StandardDeduction, RepositoryError> {
    Ok(StandardDeduction {
        tax_year: row.try_get("tax_year").map_err(db_err)?,
        filing_status_id: row.try_get("filing_status_id").map_err(db_err)?,
        amount: get_decimal(row, "amount")?,
    })
}

#[async_trait]
impl Persist<StandardDeduction> for SqliteRepository {
    async fn fetch(
        &self,
        key: &(i32, i32),
    ) -> Result<StandardDeduction, RepositoryError> {
        let (tax_year, filing_status_id) = key;
        let row = sqlx::query(
            "SELECT tax_year, filing_status_id, amount
             FROM standard_deductions
             WHERE tax_year = ? AND filing_status_id = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;
        from_row(&row)
    }

    async fn fetch_all(
        &self,
        tax_year: &i32,
    ) -> Result<Vec<StandardDeduction>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT tax_year, filing_status_id, amount
             FROM standard_deductions
             WHERE tax_year = ?
             ORDER BY filing_status_id",
        )
        .bind(tax_year)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(from_row).collect()
    }

    async fn create(
        &self,
        draft: StandardDeduction,
    ) -> Result<StandardDeduction, RepositoryError> {
        sqlx::query(
            "INSERT INTO standard_deductions (tax_year, filing_status_id, amount)
             VALUES (?, ?, ?)",
        )
        .bind(draft.tax_year)
        .bind(draft.filing_status_id)
        .bind(decimal_to_f64(draft.amount))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(draft)
    }

    async fn delete(
        &self,
        key: &(i32, i32),
    ) -> Result<(), RepositoryError> {
        let (tax_year, filing_status_id) = key;
        let result = sqlx::query(
            "DELETE FROM standard_deductions WHERE tax_year = ? AND filing_status_id = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::TaxRepository;

    use crate::repository::test_support::{clear_all_data, setup_test_db};

    use super::*;

    async fn seed(repo: &SqliteRepository) {
        clear_all_data(repo).await;
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES (9999, 200000.00, 0.125, 0.030, 0.9300, 0.55, 1500.00, 400.00)",
        )
        .execute(repo.pool())
        .await
        .expect("tax_year_config");
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (99, 'S', 'Test Single')",
        )
        .execute(repo.pool())
        .await
        .expect("filing_status");
        sqlx::query(
            "INSERT INTO standard_deductions (tax_year, filing_status_id, amount)
             VALUES (9999, 99, 18000.00)",
        )
        .execute(repo.pool())
        .await
        .expect("standard_deductions");
    }

    #[tokio::test]
    async fn get_standard_deduction() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let d = repo
            .get::<StandardDeduction>(&(9999, 99))
            .await
            .expect("found");

        assert_eq!(d.tax_year, 9999);
        assert_eq!(d.filing_status_id, 99);
        assert_eq!(d.amount, dec!(18000.00));
    }

    #[tokio::test]
    async fn get_standard_deduction_not_found() {
        let repo = setup_test_db().await;
        let result = repo.get::<StandardDeduction>(&(1999, 1)).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }
}
