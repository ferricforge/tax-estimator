use std::sync::Arc;

use anyhow::Result;
use gpui::{App, AsyncApp, BorrowAppContext, Global};
use rust_decimal::Decimal;
use tax_core::{RepositoryError, TaxEstimate, TaxRepository, TaxYearConfig, db::DbConfig};

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
    /// Wraps an existing repository implementation.
    pub fn new(repo: Arc<dyn TaxRepository>) -> Self {
        Self(repo)
    }

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

    /// Fetches the tax-year configuration for `year`.
    pub async fn get_tax_year_config(
        &self,
        year: i32,
    ) -> Result<TaxYearConfig, RepositoryError> {
        self.0.get_tax_year_config(year).await
    }

    /// Fetches a single persisted [`TaxEstimate`] by its database id.
    pub async fn get_estimate(
        &self,
        id: i64,
    ) -> Result<TaxEstimate, RepositoryError> {
        self.0.get_estimate(id).await
    }

    /// Lists persisted estimates, optionally filtered to a single tax year.
    pub async fn list_estimates(
        &self,
        tax_year: Option<i32>,
    ) -> Result<Vec<TaxEstimate>, RepositoryError> {
        self.0.list_estimates(tax_year).await
    }
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

    cx.update(|cx| cx.set_global(TaxRepo::new(repo.into())))?;
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;
    use tax_core::{
        FilingStatusCode, RepositoryError, TaxEstimateComputed, TaxEstimateInput, TaxRepository,
    };
    use tax_db_sqlite::SqliteRepository;

    use super::TaxRepo;

    async fn setup_test_repo() -> (Arc<dyn TaxRepository>, TaxRepo) {
        let seeds_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tax-db-sqlite/seeds");

        let sqlite_repo = SqliteRepository::new(":memory:")
            .await
            .expect("Failed to create in-memory database");
        sqlite_repo
            .run_migrations()
            .await
            .expect("Failed to run migrations");
        sqlite_repo
            .run_seeds(&seeds_dir)
            .await
            .expect("Failed to run seeds");

        let repo: Arc<dyn TaxRepository> = Arc::new(sqlite_repo);
        let tax_repo = TaxRepo::new(repo.clone());
        (repo, tax_repo)
    }

    fn full_input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::Single,
            se_income: Some(dec!(50000.00)),
            expected_crp_payments: Some(dec!(5000.00)),
            expected_wages: Some(dec!(60000.00)),
            expected_agi: dec!(100000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: Some(dec!(5000.00)),
            expected_amt: Some(dec!(1000.00)),
            expected_credits: Some(dec!(2000.00)),
            expected_other_taxes: Some(dec!(500.00)),
            expected_withholding: Some(dec!(8000.00)),
            prior_year_tax: Some(dec!(12000.00)),
        }
    }

    fn minimal_input() -> TaxEstimateInput {
        TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: dec!(75000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        }
    }

    #[tokio::test]
    async fn get_estimate_loads_all_optional_fields() {
        let (repo, tax_repo) = setup_test_repo().await;
        let input = full_input();

        let created = repo
            .create_estimate(input)
            .await
            .expect("create_estimate should succeed");

        let loaded = tax_repo
            .get_estimate(created.id)
            .await
            .expect("get_estimate should succeed");

        assert_eq!(loaded, created);
    }

    #[tokio::test]
    async fn get_estimate_loads_none_for_absent_optional_fields() {
        let (repo, tax_repo) = setup_test_repo().await;
        let input = minimal_input();

        let created = repo
            .create_estimate(input)
            .await
            .expect("create_estimate should succeed");

        let loaded = tax_repo
            .get_estimate(created.id)
            .await
            .expect("get_estimate should succeed");

        assert_eq!(loaded, created);
    }

    #[tokio::test]
    async fn get_estimate_loads_persisted_computed_results() {
        let (repo, tax_repo) = setup_test_repo().await;
        let input = minimal_input();

        let mut estimate = repo
            .create_estimate(input)
            .await
            .expect("create_estimate should succeed");

        let expected_computed = TaxEstimateComputed {
            se_tax: dec!(7500.00),
            total_tax: dec!(25000.00),
            required_payment: dec!(4000.00),
        };
        estimate.computed = Some(expected_computed.clone());
        repo.update_estimate(&estimate)
            .await
            .expect("update_estimate should succeed");

        let loaded = tax_repo
            .get_estimate(estimate.id)
            .await
            .expect("get_estimate should succeed");

        assert_eq!(loaded.input, estimate.input);
        assert_eq!(loaded.computed, Some(expected_computed));
    }

    #[tokio::test]
    async fn get_estimate_returns_not_found_for_missing_id() {
        let (_repo, tax_repo) = setup_test_repo().await;

        let result = tax_repo.get_estimate(99999).await;

        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn list_estimates_with_and_without_year_filter() {
        let (repo, tax_repo) = setup_test_repo().await;

        let single_input = TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::Single,
            se_income: None,
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: dec!(80000.00),
            expected_deduction: dec!(15000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        };

        let mfj_input = TaxEstimateInput {
            tax_year: 2025,
            filing_status: FilingStatusCode::MarriedFilingJointly,
            se_income: Some(dec!(40000.00)),
            expected_crp_payments: None,
            expected_wages: None,
            expected_agi: dec!(120000.00),
            expected_deduction: dec!(30000.00),
            expected_qbi_deduction: None,
            expected_amt: None,
            expected_credits: None,
            expected_other_taxes: None,
            expected_withholding: None,
            prior_year_tax: None,
        };

        repo.create_estimate(single_input)
            .await
            .expect("create single estimate");
        repo.create_estimate(mfj_input)
            .await
            .expect("create MFJ estimate");

        let all = tax_repo
            .list_estimates(None)
            .await
            .expect("list all estimates");
        assert_eq!(all.len(), 2);

        let for_2025 = tax_repo
            .list_estimates(Some(2025))
            .await
            .expect("list estimates for 2025");
        assert_eq!(for_2025.len(), 2);
        for estimate in &for_2025 {
            assert_eq!(estimate.input.tax_year, 2025);
        }

        let for_2024 = tax_repo
            .list_estimates(Some(2024))
            .await
            .expect("list estimates for 2024");
        assert_eq!(for_2024.len(), 0);
    }
}
