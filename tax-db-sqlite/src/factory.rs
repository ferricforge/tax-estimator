use std::path::Path;

use async_trait::async_trait;

use tax_core::db::repository::{RepositoryError, TaxRepository};
use tax_core::db::{DbConfig, RepositoryFactory};

use crate::repository::SqliteRepository;

/// [`RepositoryFactory`] for SQLite.
///
/// Register this with a [`tax_core::db::RepositoryRegistry`] to make the
/// `"sqlite"` backend available:
///
/// ```rust,no_run
/// use tax_core::db::RepositoryRegistry;
/// use tax_db_sqlite::SqliteRepositoryFactory;
///
/// let mut registry = RepositoryRegistry::new();
/// registry.register(Box::new(SqliteRepositoryFactory));
/// ```
pub struct SqliteRepositoryFactory;

#[async_trait]
impl RepositoryFactory for SqliteRepositoryFactory {
    fn backend_name(&self) -> &'static str {
        "sqlite"
    }

    /// Open the database described by `config.connection_string`.
    ///
    /// Accepted connection-string values:
    /// * A bare file path — e.g. `"taxes.db"`.  The file is created if it
    ///   does not exist.
    /// * `":memory:"` — an ephemeral in-memory database (useful for tests).
    ///
    /// NOTE: if your `SqliteRepository::new` expects a sqlx-style URL
    /// (`sqlite:path?mode=rwc`) rather than a bare path, adjust the
    /// mapping below accordingly.
    async fn create(
        &self,
        config: &DbConfig,
    ) -> Result<Box<dyn TaxRepository>, RepositoryError> {
        let repo = SqliteRepository::new(&config.connection_string)
            .await
            .map_err(|e| RepositoryError::Connection(format!("{e}")))?;
        repo.run_migrations()
            .await
            .map_err(|e| RepositoryError::Database(format!("{e}")))?;
        repo.run_seeds(Path::new("./seeds"))
            .await
            .map_err(|e| RepositoryError::Database(format!("{e}")))?;
        Ok(Box::new(repo))
    }
}

#[cfg(test)]
mod tests {
    use tax_core::db::DbConfig;

    use super::SqliteRepositoryFactory;
    use tax_core::db::RepositoryFactory;

    #[test]
    fn backend_name_is_sqlite() {
        assert_eq!(SqliteRepositoryFactory.backend_name(), "sqlite");
    }

    /// Full round-trip: factory → SqliteRepository with an in-memory DB.
    /// Requires that migrations are discoverable from the test's working
    /// directory.  Run from the workspace root:
    ///   cargo test -p tax-db-sqlite
    #[tokio::test]
    async fn creates_in_memory_repository() {
        let config = DbConfig {
            backend: "sqlite".to_string(),
            connection_string: ":memory:".to_string(),
        };

        let result = SqliteRepositoryFactory.create(&config).await;
        assert!(
            result.is_ok(),
            "failed to create in-memory repository: {:#?}",
            result.err()
        );
    }
}
