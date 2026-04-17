use async_trait::async_trait;
use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{Persist, RepositoryError, TaxYearConfig};

use crate::SqliteRepository;
use crate::decimal::{decimal_to_f64, get_decimal};
use crate::repository::db_err;

fn from_row(row: &SqliteRow) -> Result<TaxYearConfig, RepositoryError> {
    Ok(TaxYearConfig {
        tax_year: row.try_get("tax_year").map_err(db_err)?,
        ss_wage_max: get_decimal(row, "ss_wage_max")?,
        ss_tax_rate: get_decimal(row, "ss_tax_rate")?,
        medicare_tax_rate: get_decimal(row, "medicare_tax_rate")?,
        se_tax_deduct_pcnt: get_decimal(row, "se_tax_deductible_percentage")?,
        se_deduction_factor: get_decimal(row, "se_deduction_factor")?,
        req_pmnt_threshold: get_decimal(row, "required_payment_threshold")?,
        min_se_threshold: get_decimal(row, "min_se_threshold")?,
    })
}

#[async_trait]
impl Persist<TaxYearConfig> for SqliteRepository {
    async fn fetch(
        &self,
        year: &i32,
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
        .map_err(db_err)?
        .ok_or(RepositoryError::NotFound)?;
        from_row(&row)
    }

    async fn fetch_all(
        &self,
        _: &(),
    ) -> Result<Vec<TaxYearConfig>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                    se_tax_deductible_percentage, se_deduction_factor,
                    required_payment_threshold, min_se_threshold
             FROM tax_year_config ORDER BY tax_year DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.iter().map(from_row).collect()
    }

    async fn create(
        &self,
        draft: TaxYearConfig,
    ) -> Result<TaxYearConfig, RepositoryError> {
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(draft.tax_year)
        .bind(decimal_to_f64(draft.ss_wage_max))
        .bind(decimal_to_f64(draft.ss_tax_rate))
        .bind(decimal_to_f64(draft.medicare_tax_rate))
        .bind(decimal_to_f64(draft.se_tax_deduct_pcnt))
        .bind(decimal_to_f64(draft.se_deduction_factor))
        .bind(decimal_to_f64(draft.req_pmnt_threshold))
        .bind(decimal_to_f64(draft.min_se_threshold))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(draft)
    }

    async fn delete(
        &self,
        year: &i32,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM tax_year_config WHERE tax_year = ?")
            .bind(year)
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

    use crate::repository::test_support::setup_test_db;

    use super::*;

    async fn seed(repo: &SqliteRepository) {
        sqlx::query(
            "INSERT INTO tax_year_config (
                tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
                se_tax_deductible_percentage, se_deduction_factor,
                required_payment_threshold, min_se_threshold
            ) VALUES (9999, 200000.00, 0.125, 0.030, 0.9300, 0.55, 1500.00, 400.00)",
        )
        .execute(repo.pool())
        .await
        .expect("insert");
    }

    #[tokio::test]
    async fn get_config() {
        let repo = setup_test_db().await;
        seed(&repo).await;

        let c = repo.get::<TaxYearConfig>(&9999).await.expect("found");

        assert_eq!(c.tax_year, 9999);
        assert_eq!(c.ss_wage_max, dec!(200000.00));
        assert_eq!(c.ss_tax_rate, dec!(0.125));
        assert_eq!(c.medicare_tax_rate, dec!(0.030));
        assert_eq!(c.se_tax_deduct_pcnt, dec!(0.9300));
        assert_eq!(c.se_deduction_factor, dec!(0.55));
        assert_eq!(c.req_pmnt_threshold, dec!(1500.00));
        assert_eq!(c.min_se_threshold, dec!(400.00));
    }

    #[tokio::test]
    async fn get_config_not_found() {
        let repo = setup_test_db().await;
        let result = repo.get::<TaxYearConfig>(&1999).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn list_configs() {
        let repo = setup_test_db().await;
        seed(&repo).await;
        let configs = repo.list::<TaxYearConfig>(&()).await.expect("list");
        assert!(configs.iter().any(|c| c.tax_year == 9999));
    }
}
