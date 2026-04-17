use async_trait::async_trait;
use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{FilingStatus, FilingStatusCode, Persist, RepositoryError};

use crate::SqliteRepository;
use crate::repository::db_err;

fn from_row(row: &SqliteRow) -> Result<FilingStatus, RepositoryError> {
    let code_str: String = row.try_get("status_code").map_err(db_err)?;
    let status_code = FilingStatusCode::parse(&code_str)
        .ok_or_else(|| RepositoryError::InvalidData(format!("Invalid status code: {code_str}")))?;
    Ok(FilingStatus {
        id: row.try_get("id").map_err(db_err)?,
        status_code,
        status_name: row.try_get("status_name").map_err(db_err)?,
    })
}

#[async_trait]
impl Persist<FilingStatus> for SqliteRepository {
    async fn fetch(
        &self,
        id: &i32,
    ) -> Result<FilingStatus, RepositoryError> {
        let row =
            sqlx::query("SELECT id, status_code, status_name FROM filing_status WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?
                .ok_or(RepositoryError::NotFound)?;
        from_row(&row)
    }

    async fn fetch_all(
        &self,
        _: &(),
    ) -> Result<Vec<FilingStatus>, RepositoryError> {
        let rows =
            sqlx::query("SELECT id, status_code, status_name FROM filing_status ORDER BY id")
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?;
        rows.iter().map(from_row).collect()
    }

    async fn create(
        &self,
        draft: FilingStatus,
    ) -> Result<FilingStatus, RepositoryError> {
        sqlx::query("INSERT INTO filing_status (id, status_code, status_name) VALUES (?, ?, ?)")
            .bind(draft.id)
            .bind(draft.status_code.as_str())
            .bind(&draft.status_name)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(draft)
    }

    async fn delete(
        &self,
        id: &i32,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM filing_status WHERE id = ?")
            .bind(id)
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
    use tax_core::TaxRepository;

    use crate::repository::test_support::{clear_all_data, setup_test_db};

    use super::*;

    #[tokio::test]
    async fn list_filing_statuses() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name) VALUES
             (10, 'S', 'Test Single'),
             (20, 'MFJ', 'Test Married Filing Jointly')",
        )
        .execute(repo.pool())
        .await
        .expect("insert");

        let statuses = repo.list::<FilingStatus>(&()).await.expect("list");

        assert_eq!(statuses.len(), 2);
        let single = statuses.iter().find(|s| s.id == 10).unwrap();
        assert_eq!(single.status_code, FilingStatusCode::Single);
        assert_eq!(single.status_name, "Test Single");
        let mfj = statuses.iter().find(|s| s.id == 20).unwrap();
        assert_eq!(mfj.status_code, FilingStatusCode::MarriedFilingJointly);
        assert_eq!(mfj.status_name, "Test Married Filing Jointly");
    }

    #[tokio::test]
    async fn get_filing_status() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;
        sqlx::query(
            "INSERT INTO filing_status (id, status_code, status_name)
             VALUES (42, 'HOH', 'Test Head of Household')",
        )
        .execute(repo.pool())
        .await
        .expect("insert");

        let status = repo.get::<FilingStatus>(&42).await.expect("found");

        assert_eq!(status.id, 42);
        assert_eq!(status.status_code, FilingStatusCode::HeadOfHousehold);
        assert_eq!(status.status_name, "Test Head of Household");
    }

    #[tokio::test]
    async fn get_filing_status_not_found() {
        let repo = setup_test_db().await;
        let result = repo.get::<FilingStatus>(&999).await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }
}
