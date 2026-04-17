use async_trait::async_trait;
use rust_decimal::Decimal;
use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{Persist, RepositoryError, TaxBracket, TaxBracketFilter};

use crate::SqliteRepository;
use crate::decimal::{decimal_to_f64, get_decimal, get_optional_decimal};
use crate::repository::db_err;

fn from_row(row: &SqliteRow) -> Result<TaxBracket, RepositoryError> {
    Ok(TaxBracket {
        tax_year: row.try_get("tax_year").map_err(db_err)?,
        filing_status_id: row.try_get("filing_status_id").map_err(db_err)?,
        min_income: get_decimal(row, "min_income")?,
        max_income: get_optional_decimal(row, "max_income")?,
        tax_rate: get_decimal(row, "tax_rate")?,
        base_tax: get_decimal(row, "base_tax")?,
    })
}

#[async_trait]
impl Persist<TaxBracket> for SqliteRepository {
    async fn fetch(
        &self,
        key: &(i32, i32, Decimal),
    ) -> Result<TaxBracket, RepositoryError> {
        let (tax_year, filing_status_id, min_income) = key;
        let row = sqlx::query(
            "SELECT tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax
             FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ? AND min_income = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .bind(decimal_to_f64(*min_income))
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;
        from_row(&row)
    }

    async fn fetch_all(
        &self,
        filter: &TaxBracketFilter,
    ) -> Result<Vec<TaxBracket>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax
             FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ?
             ORDER BY min_income",
        )
        .bind(filter.tax_year)
        .bind(filter.filing_status_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(from_row).collect()
    }

    async fn create(
        &self,
        draft: TaxBracket,
    ) -> Result<TaxBracket, RepositoryError> {
        sqlx::query(
            "INSERT INTO tax_brackets
                (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(draft.tax_year)
        .bind(draft.filing_status_id)
        .bind(decimal_to_f64(draft.min_income))
        .bind(draft.max_income.map(decimal_to_f64))
        .bind(decimal_to_f64(draft.tax_rate))
        .bind(decimal_to_f64(draft.base_tax))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(draft)
    }

    async fn delete(
        &self,
        key: &(i32, i32, Decimal),
    ) -> Result<(), RepositoryError> {
        let (tax_year, filing_status_id, min_income) = key;
        let result = sqlx::query(
            "DELETE FROM tax_brackets
             WHERE tax_year = ? AND filing_status_id = ? AND min_income = ?",
        )
        .bind(tax_year)
        .bind(filing_status_id)
        .bind(decimal_to_f64(*min_income))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    async fn delete_all(
        &self,
        filter: &TaxBracketFilter,
    ) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("DELETE FROM tax_brackets WHERE tax_year = ? AND filing_status_id = ?")
                .bind(filter.tax_year)
                .bind(filter.filing_status_id)
                .execute(&self.pool)
                .await
                .map_err(db_err)?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::TaxRepository;

    use crate::repository::test_support::{clear_all_data, setup_test_db};

    use super::*;

    async fn seed_year_and_status(repo: &SqliteRepository) {
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
    }

    async fn seed_brackets(repo: &SqliteRepository) {
        seed_year_and_status(repo).await;
        sqlx::query(
            "INSERT INTO tax_brackets
                (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax)
             VALUES
                (9999, 99, 0, 10000, 0.10, 0),
                (9999, 99, 10000, 50000, 0.15, 1000),
                (9999, 99, 50000, NULL, 0.25, 7000)",
        )
        .execute(repo.pool())
        .await
        .expect("tax_brackets");
    }

    fn filter() -> TaxBracketFilter {
        TaxBracketFilter {
            tax_year: 9999,
            filing_status_id: 99,
        }
    }

    #[tokio::test]
    async fn list_brackets() {
        let repo = setup_test_db().await;
        seed_brackets(&repo).await;

        let b = repo.list::<TaxBracket>(&filter()).await.expect("list");

        assert_eq!(b.len(), 3);
        assert_eq!(b[0].min_income, dec!(0));
        assert_eq!(b[0].max_income, Some(dec!(10000)));
        assert_eq!(b[0].tax_rate, dec!(0.10));
        assert_eq!(b[0].base_tax, dec!(0));
        assert_eq!(b[1].min_income, dec!(10000));
        assert_eq!(b[1].max_income, Some(dec!(50000)));
        assert_eq!(b[1].tax_rate, dec!(0.15));
        assert_eq!(b[1].base_tax, dec!(1000));
        assert_eq!(b[2].min_income, dec!(50000));
        assert!(b[2].max_income.is_none());
        assert_eq!(b[2].tax_rate, dec!(0.25));
        assert_eq!(b[2].base_tax, dec!(7000));
    }

    #[tokio::test]
    async fn list_brackets_empty() {
        let repo = setup_test_db().await;
        let b = repo
            .list::<TaxBracket>(&TaxBracketFilter {
                tax_year: 1999,
                filing_status_id: 1,
            })
            .await
            .expect("list");
        assert!(b.is_empty());
    }

    #[tokio::test]
    async fn create_bracket() {
        let repo = setup_test_db().await;
        seed_year_and_status(&repo).await;

        TaxRepository::create::<TaxBracket>(
            &repo,
            TaxBracket {
                tax_year: 9999,
                filing_status_id: 99,
                min_income: dec!(0),
                max_income: Some(dec!(20000)),
                tax_rate: dec!(0.12),
                base_tax: dec!(0),
            },
        )
        .await
        .expect("create");

        let b = repo.list::<TaxBracket>(&filter()).await.expect("list");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].min_income, dec!(0));
        assert_eq!(b[0].max_income, Some(dec!(20000)));
        assert_eq!(b[0].tax_rate, dec!(0.12));
        assert_eq!(b[0].base_tax, dec!(0));
    }

    #[tokio::test]
    async fn create_bracket_with_null_max() {
        let repo = setup_test_db().await;
        seed_year_and_status(&repo).await;

        TaxRepository::create::<TaxBracket>(
            &repo,
            TaxBracket {
                tax_year: 9999,
                filing_status_id: 99,
                min_income: dec!(100000),
                max_income: None,
                tax_rate: dec!(0.37),
                base_tax: dec!(25000),
            },
        )
        .await
        .expect("create");

        let b = repo.list::<TaxBracket>(&filter()).await.expect("list");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].max_income, None);
    }

    #[tokio::test]
    async fn delete_matching_brackets() {
        let repo = setup_test_db().await;
        seed_brackets(&repo).await;

        let before = repo.list::<TaxBracket>(&filter()).await.expect("list");
        assert_eq!(before.len(), 3);

        let n = repo
            .delete_matching::<TaxBracket>(&filter())
            .await
            .expect("delete");
        assert_eq!(n, 3);

        let after = repo.list::<TaxBracket>(&filter()).await.expect("list");
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn delete_matching_nonexistent() {
        let repo = setup_test_db().await;
        let n = repo
            .delete_matching::<TaxBracket>(&filter())
            .await
            .expect("delete");
        assert_eq!(n, 0);
    }
}
