use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tax_core::{DataStore, RepositoryError};

pub struct SqliteRepository {
    pub(crate) pool: SqlitePool,
}

impl DataStore for SqliteRepository {}

/// Map a `sqlx::Error` to the domain error type.
pub(crate) fn db_err(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Database(e.into())
}

/// Resolve the seeds directory at runtime so it works in both development and
/// packaged distribution.
///
/// Resolution order:
/// 1. **`TAX_DB_SQLITE_SEEDS_DIR`** — if set, use this path.
/// 2. **`./seeds`** — if the directory exists in the current working directory.
/// 3. **Crate manifest dir** — `$CARGO_MANIFEST_DIR/seeds` as last resort.
fn seeds_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TAX_DB_SQLITE_SEEDS_DIR") {
        return PathBuf::from(dir);
    }
    let cwd_seeds = PathBuf::from("./seeds");
    if cwd_seeds.is_dir() {
        return cwd_seeds;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("seeds")
}

impl SqliteRepository {
    /// Open (or create) the database, run migrations, and apply seed data.
    ///
    /// This is the entry point used by `tax_db::open` and replaces the old
    /// `SqliteRepositoryFactory::create`.
    pub async fn connect(connection_string: &str) -> Result<Self, RepositoryError> {
        let repo = Self::new(connection_string)
            .await
            .map_err(RepositoryError::Connection)?;
        repo.run_migrations()
            .await
            .map_err(RepositoryError::Database)?;
        repo.run_seeds(&seeds_dir())
            .await
            .map_err(RepositoryError::Database)?;
        Ok(repo)
    }

    pub async fn new(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(database_url)
            .create_if_missing(true);

        // For :memory:, each connection gets its own DB; use a single
        // connection so migrations and seeds share the same in-memory
        // database.
        let pool = if database_url == ":memory:" {
            SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
        } else {
            SqlitePool::connect_with(options).await
        }
        .with_context(|| format!("Failed to connect to database: {}", database_url))?;

        tracing::info!("Connected to database {database_url}");
        Ok(Self { pool })
    }

    pub async fn new_with_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Apply embedded SQLx migrations to the configured database.
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        Ok(())
    }

    /// Load and execute all SQL seed files from the specified directory in
    /// alphabetical order.
    pub async fn run_seeds(
        &self,
        seeds_dir: &Path,
    ) -> Result<()> {
        tracing::info!(
            "Running seeds for sqlite from {}",
            seeds_dir.to_string_lossy()
        );

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

// ── Shared test helpers ─────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::SqliteRepository;

    pub async fn setup_test_db() -> SqliteRepository {
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

    /// Truncate all tables in dependency order.
    pub async fn clear_all_data(repo: &SqliteRepository) {
        for table in [
            "tax_estimate",
            "standard_deductions",
            "tax_brackets",
            "filing_status",
            "tax_year_config",
        ] {
            sqlx::query(&format!("DELETE FROM {table}"))
                .execute(repo.pool())
                .await
                .unwrap_or_else(|e| panic!("Failed to clear {table}: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use tax_core::{
        FilingStatus, StandardDeduction, TaxBracket, TaxBracketFilter, TaxRepository, TaxYearConfig,
    };

    use super::test_support::{clear_all_data, setup_test_db};
    use super::*;

    /// Replaces the old `creates_in_memory_repository` factory test.
    #[tokio::test]
    async fn connects_in_memory() {
        let result = SqliteRepository::connect(":memory:").await;
        assert!(
            result.is_ok(),
            "failed to create in-memory repository: {:#?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_run_seeds() {
        let repo = setup_test_db().await;
        clear_all_data(&repo).await;

        repo.run_seeds(std::path::Path::new("./seeds"))
            .await
            .expect("Should run seeds successfully");

        let statuses = repo.list::<FilingStatus>(&()).await.expect("list");
        assert_eq!(statuses.len(), 5);

        let config = repo.get::<TaxYearConfig>(&2025).await.expect("config");
        assert_eq!(config.tax_year, 2025);

        let deduction = repo
            .get::<StandardDeduction>(&(2025, 1))
            .await
            .expect("deduction");
        assert_eq!(deduction.tax_year, 2025);
        assert_eq!(deduction.filing_status_id, 1);

        let brackets = repo
            .list::<TaxBracket>(&TaxBracketFilter {
                tax_year: 2025,
                filing_status_id: 1,
            })
            .await
            .expect("brackets");
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
