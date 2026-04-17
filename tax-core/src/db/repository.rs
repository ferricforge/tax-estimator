use async_trait::async_trait;
use thiserror::Error;

use super::persist::Persist;
use super::record::TaxRecord;

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Record not found")]
    NotFound,

    #[error("Database error")]
    Database(#[source] anyhow::Error),

    #[error("Connection error")]
    Connection(#[source] anyhow::Error),

    /// Raised when required configuration is missing or a requested
    /// operation is not supported for a given record type.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// A value was retrieved from the database but could not be parsed
    /// into the expected domain type.
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// ── Configuration ───────────────────────────────────────────────────────

/// Backend-agnostic connection configuration.
///
/// | backend  | `connection_string` examples |
/// |----------|------------------------------|
/// | `sqlite` | `taxes.db`, `:memory:`       |
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbConfig {
    /// Lowercase identifier for the backend (e.g. `"sqlite"`).
    pub backend: String,
    /// Opaque value forwarded to the backend's `connect` method.
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

// ── Marker ──────────────────────────────────────────────────────────────

/// Opt-in marker for types that serve as persistence backends.
///
/// Implement this on your store type; the blanket impl below then gives
/// it the full [`TaxRepository`] API for free.
pub trait DataStore: Send + Sync {}

// ── Generic repository façade ───────────────────────────────────────────

/// Six generic persistence verbs, available on every [`DataStore`].
///
/// Individual methods become callable once the store also implements
/// [`Persist<R>`] for the record type `R` in question.  All methods are
/// provided — there is nothing to implement.
///
/// ```ignore
/// let brackets = store.list::<TaxBracket>(&filter).await?;
/// let est      = store.create::<TaxEstimate>(input).await?;
/// store.update(&est).await?;                          // R inferred
/// store.delete::<TaxEstimate>(&est.id).await?;
/// ```
#[async_trait]
pub trait TaxRepository: DataStore {
    async fn get<R: TaxRecord>(
        &self,
        key: &R::Key,
    ) -> Result<R, RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::fetch(self, key).await
    }

    async fn list<R: TaxRecord>(
        &self,
        filter: &R::Filter,
    ) -> Result<Vec<R>, RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::fetch_all(self, filter).await
    }

    async fn create<R: TaxRecord>(
        &self,
        draft: R::Draft,
    ) -> Result<R, RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::create(self, draft).await
    }

    async fn update<R: TaxRecord>(
        &self,
        record: &R,
    ) -> Result<(), RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::update(self, record).await
    }

    async fn delete<R: TaxRecord>(
        &self,
        key: &R::Key,
    ) -> Result<(), RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::delete(self, key).await
    }

    async fn delete_matching<R: TaxRecord>(
        &self,
        filter: &R::Filter,
    ) -> Result<u64, RepositoryError>
    where
        Self: Persist<R>,
    {
        <Self as Persist<R>>::delete_all(self, filter).await
    }
}

/// Every [`DataStore`] is automatically a [`TaxRepository`].
impl<S: DataStore> TaxRepository for S {}
