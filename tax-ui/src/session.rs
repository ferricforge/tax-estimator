//! Startup and connection-switch wiring: builds a repository from
//! [`AppConfig`], installs it, and keeps the cached tax year and the saved
//! settings in step.

use anyhow::Result;
use gpui::{App, AsyncApp};
use tax_core::db::{DbConfig, PoolConfig};

use crate::config::{AppConfig, RecentConnection};
use crate::repository::{TaxRepo, open_repository};
use crate::state::ActiveTaxYear;

/// The configured backend and pool settings, captured so a connection string
/// chosen later (a file-dialog result, for example) can be turned into a
/// [`DbConfig`] without another read of the app context.
///
/// Every [`DbConfig`] the application uses is built by one of its methods.
#[derive(Clone, Debug)]
pub struct DatabaseTarget {
    backend: String,
    pool: PoolConfig,
}

impl DatabaseTarget {
    /// Reads the backend and pool settings the application is configured to
    /// use.
    pub fn from_config(cx: &App) -> Self {
        let database = &AppConfig::get(cx).database;
        Self {
            backend: database.backend.as_str().to_string(),
            pool: database.pool.to_pool_config(),
        }
    }

    /// The configuration for `connection_string` on the configured backend.
    pub fn db_config(
        &self,
        connection_string: impl Into<String>,
    ) -> DbConfig {
        DbConfig {
            backend: self.backend.clone(),
            connection_string: connection_string.into(),
            pool: self.pool.clone(),
        }
    }

    /// The configuration for reopening `connection`. A recent connection
    /// carries its own backend; the pool settings are the configured ones.
    pub fn recent_db_config(
        &self,
        connection: &RecentConnection,
    ) -> DbConfig {
        DbConfig {
            backend: connection.backend.as_str().to_string(),
            connection_string: connection.url.clone(),
            pool: self.pool.clone(),
        }
    }
}

/// The database the saved settings currently point at.
pub fn configured_database(cx: &App) -> DbConfig {
    DatabaseTarget::from_config(cx).db_config(AppConfig::get(cx).database.url.clone())
}

/// The configured database location, when it names a database that does not
/// exist. Opening it would silently create an empty one.
pub fn missing_configured_database(cx: &App) -> Option<String> {
    let database = &AppConfig::get(cx).database;
    (!database.backend.location_exists(&database.url)).then(|| database.url.clone())
}

/// Opens the configured database and installs it as the process-wide handle.
/// Call once during startup, after [`AppConfig`](crate::config::AppConfig) is
/// installed as a gpui global.
pub async fn init_database(cx: &mut AsyncApp) -> Result<()> {
    let db_config = cx.update(|cx| configured_database(cx))?;
    let repo = open_repository(&db_config).await?;
    cx.update(|cx| TaxRepo::install(repo, cx))?;
    Ok(())
}

/// Points the application at a different database.
///
/// Installs a new [`TaxRepo`], discards the cached [`ActiveTaxYear`], records
/// the connection details in [`AppConfig`], and adds the connection that was
/// open before to the recent list. Nothing is recorded when the new database
/// cannot be opened.
///
/// Shared by the *New Connection*, *Open Connection*, and *Save As* actions.
pub async fn switch_database(
    cx: &mut AsyncApp,
    db_config: DbConfig,
) -> Result<()> {
    let repo = open_repository(&db_config).await?;
    cx.update(|cx| {
        TaxRepo::install(repo, cx);
        ActiveTaxYear::reset(cx);
        remember_database(cx, &db_config);
    })?;
    tracing::info!(
        backend = %db_config.backend,
        connection_string = %db_config.connection_string,
        "switched active database"
    );
    Ok(())
}

/// Removes recent connections whose databases no longer exist, and writes
/// the settings file when anything was removed.
pub fn forget_missing_recent_connections(cx: &mut App) {
    let removed = AppConfig::update(cx, |cfg| cfg.recent.remove_missing());
    if removed == 0 {
        return;
    }

    tracing::info!(removed, "removed recent connections whose files no longer exist");
    if let Err(e) = AppConfig::save(cx) {
        tracing::warn!(error = %e, "failed to persist config after updating recent connections");
    }
}

/// Records `db_config` in [`AppConfig`], updates the recent list, and writes
/// the settings file.
///
/// The connection that was open before is listed only if it still exists, so
/// a configured database that was never found is not remembered.
///
/// Only the location and backend change. The pool settings in `db_config`
/// were read from [`AppConfig`] in the first place, so they stay as they are.
fn remember_database(
    cx: &mut App,
    db_config: &DbConfig,
) {
    AppConfig::update(cx, |cfg| {
        let previous = RecentConnection::for_database(cfg.database.backend, &cfg.database.url)
            .filter(RecentConnection::exists);

        cfg.database.url = db_config.connection_string.clone();
        if let Ok(backend) = db_config.backend.parse() {
            cfg.database.backend = backend;
        }

        let current = RecentConnection::for_database(cfg.database.backend, &cfg.database.url);
        cfg.recent.record_switch(previous, current.as_ref());
    });
    if let Err(e) = AppConfig::save(cx) {
        tracing::warn!(error = %e, "failed to persist config after switching database");
    }
}
