use async_trait::async_trait;
use tax_core::db::repository::{RepositoryError, TaxRepository};
use tax_core::db::{DbConfig, RepositoryFactory};

use crate::repository::SqliteRepository;
use crate::seeds;

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

    /// Open the database described by `config.connection_string`, with a
    /// connection pool built from `config.pool`.
    ///
    /// Accepted connection-string values:
    /// * A bare file path — e.g. `"taxes.db"`.  The file is created if it
    ///   does not exist.
    /// * `":memory:"` — an ephemeral in-memory database (useful for tests).
    ///   It always uses one permanent connection, whatever `config.pool` says.
    ///
    /// After connecting, migrations are applied and then every seed script
    /// embedded at build time (see [`crate::seeds`]) is run, in file-name
    /// order. Nothing is read from the filesystem, so behaviour is identical
    /// in development, in tests, and in a packaged binary.
    async fn create(
        &self,
        config: &DbConfig,
    ) -> Result<Box<dyn TaxRepository>, RepositoryError> {
        let database_url = &config.connection_string;
        let repo = SqliteRepository::new_with_pool_config(database_url, &config.pool)
            .await
            .map_err(RepositoryError::Connection)?;
        repo.run_migrations()
            .await
            .map_err(RepositoryError::Database)?;
        repo.run_seeds(seeds::embedded())
            .await
            .map_err(RepositoryError::Database)?;
        Ok(Box::new(repo))
    }
}

#[cfg(test)]
mod tests {
    use tax_core::db::{DbConfig, RepositoryFactory};

    use super::SqliteRepositoryFactory;

    fn in_memory_config() -> DbConfig {
        DbConfig {
            backend: "sqlite".to_string(),
            connection_string: ":memory:".to_string(),
            ..DbConfig::default()
        }
    }

    #[test]
    fn backend_name_is_sqlite() {
        assert_eq!(SqliteRepositoryFactory.backend_name(), "sqlite");
    }

    /// Full round-trip: factory → migrations → embedded seeds, against an
    /// in-memory database. Nothing here depends on the working directory.
    #[tokio::test]
    async fn creates_in_memory_repository() {
        let result = SqliteRepositoryFactory.create(&in_memory_config()).await;

        assert!(
            result.is_ok(),
            "failed to create in-memory repository: {:#?}",
            result.err()
        );
    }

    /// Proves the seeds actually ran: a freshly created database must already
    /// hold reference data for the shipped tax year.
    #[tokio::test]
    async fn created_repository_is_seeded() {
        let repo = SqliteRepositoryFactory
            .create(&in_memory_config())
            .await
            .expect("in-memory repository should be created");

        let statuses = repo
            .get_filing_status_data(2025)
            .await
            .expect("seeded reference data should load");

        assert!(
            !statuses.is_empty(),
            "seeds should populate filing statuses, deductions and brackets for 2025"
        );
    }
}
