use std::sync::Arc;

use anyhow::Result;
use gpui::{App, AsyncApp, BorrowAppContext, Global};
use rust_decimal::Decimal;
use tax_core::{RepositoryError, TaxRepository, TaxYearConfig, db::DbConfig}; // adjust path as needed

use crate::{
    app::{TaxYearData, build_registry, load_tax_year_data},
    config::AppConfig,
};

// ---------------------------------------------------------------------------
// Shared repository handle
// ---------------------------------------------------------------------------

/// Process-wide database handle. Cheap to clone; holds an `Arc`.
#[derive(Clone)]
pub struct TaxRepo(Arc<dyn TaxRepository>);

impl Global for TaxRepo {}

impl TaxRepo {
    pub fn get(cx: &App) -> Self {
        cx.global::<Self>().clone()
    }

    pub fn try_get(cx: &App) -> Option<Self> {
        cx.try_global::<Self>().cloned()
    }

    pub fn tax_repository(&self) -> &dyn TaxRepository {
        &*self.0
    }

    pub fn tax_repository_arc(&self) -> Arc<dyn TaxRepository> {
        self.0.clone()
    }

    pub async fn get_tax_year_config(
        &self,
        year: i32,
    ) -> Result<TaxYearConfig, RepositoryError> {
        self.0.get_tax_year_config(year).await
    }

    // Add more delegating methods as you need them.
}

/// Build the repository from `AppConfig` and install it as a global.
/// Call once during startup, *after* `AppConfig::init`.
pub async fn init_repository(cx: &mut gpui::AsyncApp) -> Result<()> {
    let (url, backend) = cx.update(|cx| {
        let cfg = AppConfig::get(cx);
        (cfg.database_url.clone(), cfg.database_backend.as_str())
    })?;

    let db_config = DbConfig {
        backend: backend.to_string(),
        connection_string: url,
    };

    let registry = build_registry();
    let repo = registry.create(&db_config).await?;

    cx.update(|cx| cx.set_global(TaxRepo(repo.into())))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Active tax year (config loaded on demand)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct ActiveTaxYear {
    pub year: Option<i32>,
    pub tax_year_data: Option<TaxYearData>,
}

impl Global for ActiveTaxYear {}

impl ActiveTaxYear {
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn ss_wage_max(cx: &App) -> Option<Decimal> {
        cx.try_global::<Self>()
            .and_then(|a: &ActiveTaxYear| a.tax_year_data.as_ref())
            .map(|tyd: &TaxYearData| tyd.config.ss_wage_max)
    }

    /// Kick off a fetch for `year`. No-op if already loaded.
    pub fn load(
        year: i32,
        cx: &mut App,
    ) {
        if cx
            .try_global::<Self>()
            .map(|a| a.year == Some(year) && a.tax_year_data.is_some())
            .unwrap_or(false)
        {
            tracing::info!("Already have tax year");
            return; // already have it
        }

        let Some(repo) = TaxRepo::try_get(cx) else {
            tracing::warn!("TaxRepo not initialised; cannot load tax year {year}");
            return;
        };

        tracing::info!("Setting global year {year}");
        cx.set_global(Self {
            year: Some(year),
            tax_year_data: None,
        });

        cx.spawn(async move |async_cx: &mut AsyncApp| {
            match load_tax_year_data(repo.tax_repository(), year).await {
                // match repo.get_tax_year_config(year).await {
                Ok(tax_year_data) => {
                    let _ = async_cx.update(|cx| {
                        // update_global notifies observe_global subscribers
                        cx.update_global::<Self, _>(|active, _| {
                            active.year = Some(year);
                            active.tax_year_data = Some(tax_year_data);
                            tracing::trace!("Tax year load: {:#?}", active);
                        });
                    });
                }
                Err(e) => {
                    tracing::warn!(%e, year, "Failed to load TaxYearConfig");
                }
            }
        })
        .detach();
    }
}
