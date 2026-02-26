use std::collections::HashMap;

use async_trait::async_trait;

use super::repository::{RepositoryError, TaxRepository};

/// Backend-agnostic connection configuration.
///
/// `backend` must match the [`RepositoryFactory::backend_name`] of a
/// registered factory.  `connection_string` is passed through to that
/// factory unchanged — its meaning is entirely backend-specific.
///
/// | backend    | connection_string examples          |
/// |------------|-------------------------------------|
/// | `sqlite`   | `taxes.db`, `:memory:`              |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbConfig {
    /// Lowercase identifier matching a registered factory (e.g. `"sqlite"`).
    pub backend: String,
    /// Opaque value forwarded to the factory's `create` method.
    pub connection_string: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            backend: "sqlite".to_string(),
            connection_string: ":memory:".to_string(),
        }
    }
}

/// One implementation per database backend.  Each backend crate exports a
/// single unit struct that implements this trait and is registered with a
/// [`RepositoryRegistry`] at startup.
#[async_trait]
pub trait RepositoryFactory: Send + Sync {
    /// Unique, lowercase identifier for this backend.
    fn backend_name(&self) -> &'static str;

    /// Open (or create) a connection and return a ready-to-use repository.
    /// Implementations are free to run migrations or warm connection pools
    /// inside this method.
    async fn create(
        &self,
        config: &DbConfig,
    ) -> Result<Box<dyn TaxRepository>, RepositoryError>;
}

/// Registry of [`RepositoryFactory`] instances, keyed by backend name.
///
/// Typical lifetime:
/// 1. Create with `RepositoryRegistry::new()`.
/// 2. Call `register` once per known backend.
/// 3. Call `create` whenever a new repository is needed.
pub struct RepositoryRegistry {
    factories: HashMap<&'static str, Box<dyn RepositoryFactory>>,
}

impl RepositoryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Register a backend factory.
    ///
    /// If a factory with the same [`RepositoryFactory::backend_name`] is
    /// already present it is silently replaced.
    pub fn register(
        &mut self,
        factory: Box<dyn RepositoryFactory>,
    ) {
        self.factories.insert(factory.backend_name(), factory);
    }

    /// Names of every registered backend, sorted alphabetically.
    pub fn available_backends(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.factories.keys().copied().collect();
        names.sort_unstable();
        names
    }

    /// Dispatch to the factory that matches `config.backend` and return
    /// the repository it produces.
    ///
    /// # Errors
    /// * [`RepositoryError::Configuration`] — no factory is registered for
    ///   the requested backend name.
    /// * Any error the chosen factory itself returns.
    pub async fn create(
        &self,
        config: &DbConfig,
    ) -> Result<Box<dyn TaxRepository>, RepositoryError> {
        let factory = self.factories.get(config.backend.as_str()).ok_or_else(|| {
            RepositoryError::Configuration(format!(
                "unknown backend '{}'; available: {:?}",
                config.backend,
                self.available_backends()
            ))
        })?;

        factory.create(config).await
    }
}

impl Default for RepositoryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;

    use crate::models::{
        FilingStatus, NewTaxEstimate, StandardDeduction, TaxBracket, TaxEstimate, TaxYearConfig,
    };

    use super::{DbConfig, RepositoryError, RepositoryFactory, RepositoryRegistry, TaxRepository};

    // ── test scaffolding ─────────────────────────────────────────────────
    // Every method panics unconditionally.  Nothing in these tests actually
    // calls through to the repository; they only exercise registry routing.

    struct StubRepository;

    #[async_trait]
    impl TaxRepository for StubRepository {
        async fn get_tax_year_config(
            &self,
            _year: i32,
        ) -> Result<TaxYearConfig, RepositoryError> {
            unimplemented!()
        }
        async fn list_tax_years(&self) -> Result<Vec<i32>, RepositoryError> {
            unimplemented!()
        }
        async fn get_filing_status(
            &self,
            _id: i32,
        ) -> Result<FilingStatus, RepositoryError> {
            unimplemented!()
        }
        async fn get_filing_status_by_code(
            &self,
            _code: &str,
        ) -> Result<FilingStatus, RepositoryError> {
            unimplemented!()
        }
        async fn list_filing_statuses(&self) -> Result<Vec<FilingStatus>, RepositoryError> {
            unimplemented!()
        }
        async fn get_standard_deduction(
            &self,
            _tax_year: i32,
            _filing_status_id: i32,
        ) -> Result<StandardDeduction, RepositoryError> {
            unimplemented!()
        }
        async fn get_tax_brackets(
            &self,
            _tax_year: i32,
            _filing_status_id: i32,
        ) -> Result<Vec<TaxBracket>, RepositoryError> {
            unimplemented!()
        }
        async fn insert_tax_bracket(
            &self,
            _bracket: &TaxBracket,
        ) -> Result<(), RepositoryError> {
            unimplemented!()
        }
        async fn delete_tax_brackets(
            &self,
            _tax_year: i32,
            _filing_status_id: i32,
        ) -> Result<(), RepositoryError> {
            unimplemented!()
        }
        async fn create_estimate(
            &self,
            _estimate: NewTaxEstimate,
        ) -> Result<TaxEstimate, RepositoryError> {
            unimplemented!()
        }
        async fn get_estimate(
            &self,
            _id: i64,
        ) -> Result<TaxEstimate, RepositoryError> {
            unimplemented!()
        }
        async fn update_estimate(
            &self,
            _estimate: &TaxEstimate,
        ) -> Result<(), RepositoryError> {
            unimplemented!()
        }
        async fn delete_estimate(
            &self,
            _id: i64,
        ) -> Result<(), RepositoryError> {
            unimplemented!()
        }
        async fn list_estimates(
            &self,
            _tax_year: Option<i32>,
        ) -> Result<Vec<TaxEstimate>, RepositoryError> {
            unimplemented!()
        }
    }

    /// Records whether `create` was called.  The `AtomicBool` is the
    /// only way tests can observe side-effects of dispatch.
    struct StubFactory {
        name: &'static str,
        called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl RepositoryFactory for StubFactory {
        fn backend_name(&self) -> &'static str {
            self.name
        }
        async fn create(
            &self,
            _config: &DbConfig,
        ) -> Result<Box<dyn TaxRepository>, RepositoryError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(Box::new(StubRepository))
        }
    }

    /// Always returns a `Connection` error — used to verify that
    /// errors from the underlying factory are surfaced unchanged.
    struct FailingFactory;

    #[async_trait]
    impl RepositoryFactory for FailingFactory {
        fn backend_name(&self) -> &'static str {
            "failing"
        }
        async fn create(
            &self,
            _config: &DbConfig,
        ) -> Result<Box<dyn TaxRepository>, RepositoryError> {
            Err(RepositoryError::Connection(anyhow::anyhow!(
                "intentional failure".to_string()
            )))
        }
    }

    fn stub_factory(name: &'static str) -> (Box<dyn RepositoryFactory>, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (
            Box::new(StubFactory {
                name,
                called: flag.clone(),
            }),
            flag,
        )
    }

    /// `Box<dyn TaxRepository>` implements neither `Debug` nor `PartialEq`,
    /// so the standard `Result` helpers (`unwrap_err`, `assert_eq!`) are
    /// unavailable on the full `Result` type.  Pull the error out manually
    /// before any formatting or comparison happens.
    fn expect_error(result: Result<Box<dyn TaxRepository>, RepositoryError>) -> RepositoryError {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err, got Ok"),
        }
    }

    // ── DbConfig ─────────────────────────────────────────────────────────

    #[test]
    fn dbconfig_default_is_sqlite_memory() {
        let cfg = DbConfig::default();
        assert_eq!(cfg.backend, "sqlite");
        assert_eq!(cfg.connection_string, ":memory:");
    }

    // ── registry construction ────────────────────────────────────────────

    #[test]
    fn new_registry_has_no_backends() {
        assert!(RepositoryRegistry::new().available_backends().is_empty());
    }

    #[test]
    fn default_registry_is_empty() {
        // Default is part of the public API contract; test it explicitly
        // even though the impl delegates to new().
        assert!(
            RepositoryRegistry::default()
                .available_backends()
                .is_empty()
        );
    }

    // ── registration ─────────────────────────────────────────────────────

    #[test]
    fn register_single_backend() {
        let mut reg = RepositoryRegistry::new();
        let (factory, _) = stub_factory("sqlite");
        reg.register(factory);
        assert_eq!(reg.available_backends(), vec!["sqlite"]);
    }

    #[test]
    fn available_backends_is_sorted() {
        let mut reg = RepositoryRegistry::new();
        // Register in deliberately reverse-alphabetical order.
        let (f1, _) = stub_factory("sqlite");
        let (f2, _) = stub_factory("postgres");
        reg.register(f1);
        reg.register(f2);
        assert_eq!(reg.available_backends(), vec!["postgres", "sqlite"]);
    }

    #[test]
    fn duplicate_registration_replaces() {
        let mut reg = RepositoryRegistry::new();
        let (old, _) = stub_factory("sqlite");
        let (new, _) = stub_factory("sqlite");
        reg.register(old);
        reg.register(new);
        // Only one entry should remain.
        assert_eq!(reg.available_backends(), vec!["sqlite"]);
    }

    // ── dispatch ─────────────────────────────────────────────────────────

    /// With two backends registered, dispatching to "sqlite" must invoke
    /// the sqlite factory and must NOT invoke the postgres factory.
    ///
    /// This replaces two previous tests: one that only checked the
    /// positive flag, and one that *claimed* to check the negative flag
    /// but never actually read it.
    #[tokio::test]
    async fn create_dispatches_to_matching_factory_only() {
        let mut reg = RepositoryRegistry::new();
        let (sqlite_f, sqlite_called) = stub_factory("sqlite");
        let (postgres_f, postgres_called) = stub_factory("postgres");
        reg.register(sqlite_f);
        reg.register(postgres_f);

        let config = DbConfig {
            backend: "sqlite".to_string(),
            connection_string: ":memory:".to_string(),
        };

        assert!(reg.create(&config).await.is_ok());

        assert!(
            sqlite_called.load(Ordering::SeqCst),
            "sqlite factory must be called"
        );
        assert!(
            !postgres_called.load(Ordering::SeqCst),
            "postgres factory must NOT be called"
        );
    }

    // ── unknown backend ──────────────────────────────────────────────────

    /// Empty registry: any backend name produces a Configuration error
    /// that names the requested backend.
    #[tokio::test]
    async fn unknown_backend_returns_configuration_error() {
        let reg = RepositoryRegistry::new();
        let config = DbConfig {
            backend: "nope".to_string(),
            connection_string: "x".to_string(),
        };

        match expect_error(reg.create(&config).await) {
            RepositoryError::Configuration(msg) => {
                assert!(
                    msg.contains("nope"),
                    "message should name the requested backend"
                );
            }
            other => panic!("expected Configuration, got {other:#?}"),
        }
    }

    /// Populated registry: the Configuration error also lists the
    /// backends that *are* available.  Different scenario from the test
    /// above (empty registry produces `available: []`; this one does not).
    #[tokio::test]
    async fn configuration_error_lists_available_backends() {
        let mut reg = RepositoryRegistry::new();
        let (f, _) = stub_factory("sqlite");
        reg.register(f);

        let config = DbConfig {
            backend: "postgres".to_string(),
            connection_string: "x".to_string(),
        };

        match expect_error(reg.create(&config).await) {
            RepositoryError::Configuration(msg) => {
                assert!(
                    msg.contains("postgres"),
                    "message should name the requested backend"
                );
                assert!(
                    msg.contains("sqlite"),
                    "message should list available backends"
                );
            }
            other => panic!("expected Configuration, got {other:#?}"),
        }
    }

    // ── factory errors propagate ─────────────────────────────────────────

    #[tokio::test]
    async fn factory_error_propagates() {
        let mut reg = RepositoryRegistry::new();
        reg.register(Box::new(FailingFactory));

        let config = DbConfig {
            backend: "failing".to_string(),
            connection_string: "x".to_string(),
        };

        // expect_error extracts the RepositoryError first; both sides of
        // assert_eq! are now plain RepositoryError, which is Debug + PartialEq.
        let err = expect_error(reg.create(&config).await);
        match err {
            RepositoryError::Connection(inner) => {
                assert!(
                    inner.to_string().contains("intentional failure"),
                    "unexpected error message: {inner}"
                );
            }
            other => panic!("expected RepositoryError::Connection, got {other:?}"),
        }
    }
}
