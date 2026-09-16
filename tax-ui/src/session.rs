//! Startup and project-switch wiring: builds a repository from [`AppConfig`],
//! installs it, and keeps the cached tax year and the saved settings in step.

use anyhow::Result;
use gpui::{App, AsyncApp};
use tax_core::db::DbConfig;

use crate::config::AppConfig;
use crate::repository::{TaxRepo, open_repository};
use crate::state::ActiveTaxYear;

/// The configured backend, captured so a connection string chosen later (a
/// file-dialog result, for example) can be turned into a [`DbConfig`] without
/// another read of the app context.
#[derive(Clone, Debug)]
pub struct DatabaseTarget {
    backend: String,
}

impl DatabaseTarget {
    /// Reads the backend the application is configured to use.
    pub fn from_config(cx: &App) -> Self {
        Self {
            backend: AppConfig::get(cx).database_backend.as_str().to_string(),
        }
    }

    /// The only place a [`DbConfig`] is built.
    pub fn db_config(
        &self,
        connection_string: impl Into<String>,
    ) -> DbConfig {
        DbConfig {
            backend: self.backend.clone(),
            connection_string: connection_string.into(),
        }
    }
}

/// The database the saved settings currently point at.
pub fn configured_database(cx: &App) -> DbConfig {
    DatabaseTarget::from_config(cx).db_config(AppConfig::get(cx).database_url.clone())
}

/// Opens the configured database and installs it as the process-wide handle.
/// Call once during startup, after `AppConfig::init`.
pub async fn init_database(cx: &mut AsyncApp) -> Result<()> {
    let db_config = cx.update(|cx| configured_database(cx))?;
    let repo = open_repository(&db_config).await?;
    cx.update(|cx| TaxRepo::install(repo, cx))?;
    Ok(())
}

/// Points the application at a different database.
///
/// Installs a new [`TaxRepo`], discards the cached [`ActiveTaxYear`], and
/// records the connection details in [`AppConfig`].
///
/// Shared by the project *Open*, *New*, and *Save As* actions.
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

/// Records `db_config` in [`AppConfig`] and writes the settings file.
fn remember_database(
    cx: &mut App,
    db_config: &DbConfig,
) {
    AppConfig::update(cx, |cfg| {
        cfg.database_url = db_config.connection_string.clone();
        if let Ok(backend) = db_config.backend.parse() {
            cfg.database_backend = backend;
        }
    });
    if let Err(e) = AppConfig::save(cx) {
        tracing::warn!(error = %e, "failed to persist config after switching database");
    }
}
