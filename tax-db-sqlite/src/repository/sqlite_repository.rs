use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::{
    AssertSqlSafe,
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
};
use tax_core::{FilingStatusCode, RepositoryError, TaxRepository, db::PoolConfig};

use crate::seeds::Seed;

/// Connection string that selects a private in-memory database.
const IN_MEMORY_URL: &str = ":memory:";

/// Pool value meaning "never close a connection for this reason".
const NO_EXPIRY: Option<Duration> = None;

/// SQLite-backed implementation of [`tax_core::TaxRepository`].
///
/// Wraps a connection pool and owns the schema migrations and reference-data
/// seeds for the `"sqlite"` backend. Construct one with [`SqliteRepository::new`]
/// or [`SqliteRepository::new_with_pool_config`] (or
/// [`SqliteRepository::new_with_pool`] in tests), then call
/// `[run_migrations](Self::run_migrations)` and, when reference data is needed,
/// `[run_seeds](Self::run_seeds)` before use.
pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    /// Open (creating it if missing) the database at `database_url` with the
    /// default pool settings.
    ///
    /// See `[new_with_pool_config](Self::new_with_pool_config)` for how
    /// `database_url` is interpreted.
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_pool_config(database_url, &PoolConfig::default()).await
    }

    /// Open (creating it if missing) the database at `database_url` and return
    /// a repository backed by a connection pool built from `pool_config`.
    ///
    /// The special value `":memory:"` opens a single-connection in-memory
    /// database so that migrations and seeds operate on the same storage; any
    /// other value is treated as a filesystem path. For `":memory:"` the
    /// connection limits and expiry settings in `pool_config` are ignored.
    pub async fn new_with_pool_config(
        database_url: &str,
        pool_config: &PoolConfig,
    ) -> Result<Self> {
        let connect_options = SqliteConnectOptions::new()
            .filename(database_url)
            .create_if_missing(true);

        let pool = pool_options(database_url, pool_config)
            .connect_with(connect_options)
            .await
            .with_context(|| format!("Failed to connect to database: {database_url}"))?;

        tracing::info!("Connected to database {database_url}");
        Ok(Self { pool })
    }

    /// Build a repository from an already-configured [`SqlitePool`].
    ///
    /// Useful for tests that construct the pool themselves (for example a
    /// shared in-memory database).
    pub async fn new_with_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Apply the embedded SQLx migrations to the configured database.
    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        Ok(())
    }

    /// Applies every seed script in order. The factory runs this on every
    /// open, so the scripts must remain idempotent.
    pub async fn run_seeds(
        &self,
        seeds: &[Seed],
    ) -> anyhow::Result<()> {
        for seed in seeds {
            tracing::debug!(seed = seed.name, "applying seed");
            sqlx::raw_sql(seed.sql)
                .execute(&self.pool)
                .await
                .with_context(|| format!("failed to apply seed '{}'", seed.name))?;
        }
        Ok(())
    }

    /// Borrow the underlying connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Collapse the write-ahead log into the main database file.
    ///
    /// Estimates are committed as they are calculated, so nothing new is
    /// persisted here; this just leaves the on-disk `.db` self-contained
    /// (no `-wal`/`-shm` sidecar) for the *Save* action.
    pub async fn checkpoint(&self) -> Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .context("Failed to checkpoint the write-ahead log")?;
        Ok(())
    }

    /// Write a consistent, fully self-contained copy of the database to
    /// `dest`, including every committed estimate and all reference data.
    ///
    /// Uses SQLite's `VACUUM INTO`, so `dest` is a single clean file with no
    /// `-wal`/`-shm` sidecars regardless of the source journal mode. `dest`
    /// must not already exist.
    pub async fn backup_to(
        &self,
        dest: &Path,
    ) -> Result<()> {
        let dest = dest.to_str().ok_or_else(|| {
            anyhow::anyhow!("Destination path is not valid UTF-8: {}", dest.display())
        })?;

        // `VACUUM INTO` only accepts a string literal, never a bound parameter.
        let sql = format!("VACUUM INTO '{}'", dest.replace('\'', "''"));
        sqlx::raw_sql(AssertSqlSafe(sql))
            .execute(&self.pool)
            .await
            .with_context(|| format!("Failed to copy the database to '{dest}'"))?;
        Ok(())
    }

    /// Resolve the stored `filing_status` row id for `code`.
    pub(crate) async fn filing_status_id_for_code(
        &self,
        code: FilingStatusCode,
    ) -> Result<i32, RepositoryError> {
        Ok(self.get_filing_status_by_code(code.as_str()).await?.id)
    }
}

/// Builds the SQLx pool options for `database_url` from `pool_config`.
///
/// Values a pool cannot use are replaced first (see
/// [`PoolConfig::normalized`]) and the replacement is logged.
fn pool_options(
    database_url: &str,
    pool_config: &PoolConfig,
) -> SqlitePoolOptions {
    let effective = pool_config.normalized();
    if effective != *pool_config {
        tracing::warn!(
            requested = ?pool_config,
            applied = ?effective,
            "Database pool settings were adjusted to usable values"
        );
    }

    let options = SqlitePoolOptions::new()
        .acquire_timeout(effective.acquire_timeout)
        .test_before_acquire(effective.test_before_acquire);

    if database_url == IN_MEMORY_URL {
        // Every connection to `:memory:` gets its own database, and that
        // database is destroyed when its connection closes. Keep exactly one
        // connection and never let the pool close it for idleness or age.
        return options
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(NO_EXPIRY)
            .max_lifetime(NO_EXPIRY);
    }

    options
        .max_connections(effective.max_connections)
        .min_connections(effective.min_connections)
        .idle_timeout(effective.idle_timeout)
        .max_lifetime(effective.max_lifetime)
}

#[cfg(test)]
mod pool_options_tests {
    use std::time::Duration;

    use tax_core::db::PoolConfig;

    use super::{IN_MEMORY_URL, pool_options};

    #[test]
    fn in_memory_keeps_one_permanent_connection() {
        let requested = PoolConfig {
            max_connections: 8,
            ..PoolConfig::default()
        };

        let options = pool_options(IN_MEMORY_URL, &requested);

        assert_eq!(options.get_max_connections(), 1);
        assert_eq!(options.get_min_connections(), 1);
        assert_eq!(options.get_idle_timeout(), None);
        assert_eq!(options.get_max_lifetime(), None);
    }

    #[test]
    fn file_database_uses_configured_values() {
        let requested = PoolConfig {
            max_connections: 4,
            min_connections: 2,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: None,
            max_lifetime: Some(Duration::from_secs(120)),
            test_before_acquire: false,
        };

        let options = pool_options("taxes.db", &requested);

        assert_eq!(options.get_max_connections(), 4);
        assert_eq!(options.get_min_connections(), 2);
        assert_eq!(options.get_acquire_timeout(), Duration::from_secs(5));
        assert_eq!(options.get_idle_timeout(), None);
        assert_eq!(options.get_max_lifetime(), Some(Duration::from_secs(120)));
        assert!(!options.get_test_before_acquire());
    }
}
