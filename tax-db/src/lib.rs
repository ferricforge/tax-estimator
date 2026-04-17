use async_trait::async_trait;
use tax_db_sqlite::SqliteRepository;

// Re-export consumer-facing types so tax-ui can depend on tax-db alone
// for its repository plumbing.
pub use tax_core::db::{DbConfig, Persist, RepositoryError, TaxRecord, TaxRepository};

use tax_core::db::{DataStore, RepositoryError as RepoError, TaxRecord as Record};

/// Enum-dispatched store that routes to whichever backend was selected at
/// startup.  Adding a backend means one new variant, one match arm in the
/// macro, and one extra bound on the blanket impl.
pub enum TaxStore {
    Sqlite(SqliteRepository),
}

impl DataStore for TaxStore {}

/// Dispatches a method call to the inner backend.
macro_rules! dispatch {
    ($self:ident, $s:ident => $body:expr) => {
        match $self {
            TaxStore::Sqlite($s) => $body,
        }
    };
}

/// Blanket: if every compiled-in backend can persist `R`, so can `TaxStore`.
#[async_trait]
impl<R: Record> Persist<R> for TaxStore
where
    SqliteRepository: Persist<R>,
{
    async fn fetch(
        &self,
        key: &R::Key,
    ) -> Result<R, RepoError> {
        dispatch!(self, s => Persist::<R>::fetch(s, key).await)
    }

    async fn fetch_all(
        &self,
        filter: &R::Filter,
    ) -> Result<Vec<R>, RepoError> {
        dispatch!(self, s => Persist::<R>::fetch_all(s, filter).await)
    }

    async fn create(
        &self,
        draft: R::Draft,
    ) -> Result<R, RepoError> {
        dispatch!(self, s => Persist::<R>::create(s, draft).await)
    }

    async fn update(
        &self,
        record: &R,
    ) -> Result<(), RepoError> {
        dispatch!(self, s => Persist::<R>::update(s, record).await)
    }

    async fn delete(
        &self,
        key: &R::Key,
    ) -> Result<(), RepoError> {
        dispatch!(self, s => Persist::<R>::delete(s, key).await)
    }

    async fn delete_all(
        &self,
        filter: &R::Filter,
    ) -> Result<u64, RepoError> {
        dispatch!(self, s => Persist::<R>::delete_all(s, filter).await)
    }
}

/// Open a [`TaxStore`] from the provided configuration.
///
/// Replaces the old `RepositoryRegistry` / `RepositoryFactory` pair.
pub async fn open(config: &DbConfig) -> Result<TaxStore, RepoError> {
    match config.backend.as_str() {
        "sqlite" => {
            let repo = SqliteRepository::connect(&config.connection_string).await?;
            Ok(TaxStore::Sqlite(repo))
        }
        other => Err(RepoError::Configuration(format!(
            "unknown backend '{other}'"
        ))),
    }
}
