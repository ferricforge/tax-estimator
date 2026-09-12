use std::path::Path;

use anyhow::{Context, Result};
use sqlx::{
    AssertSqlSafe,
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
};
use tax_core::{FilingStatusCode, RepositoryError, TaxRepository};

use crate::seeds::Seed;

/// SQLite-backed implementation of [`tax_core::TaxRepository`].
///
/// Wraps a connection pool and owns the schema migrations and reference-data
/// seeds for the `"sqlite"` backend. Construct one with [`SqliteRepository::new`]
/// (or [`SqliteRepository::new_with_pool`] in tests), then call
/// [`run_migrations`](Self::run_migrations) and, when reference data is needed,
/// [`run_seeds`](Self::run_seeds) before use.
pub struct SqliteRepository {
    pool: SqlitePool,
}

impl SqliteRepository {
    /// Open (creating it if missing) the database at `database_url` and return
    /// a repository backed by a connection pool.
    ///
    /// The special value `":memory:"` opens a single-connection in-memory
    /// database so that migrations and seeds operate on the same storage; any
    /// other value is treated as a filesystem path.
    pub async fn new(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(database_url)
            .create_if_missing(true);

        // For :memory:, each connection gets its own DB; use a single connection
        // so migrations and seeds run against the same in-memory database.
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
