use async_trait::async_trait;

use super::record::TaxRecord;
use super::repository::RepositoryError;

/// Backend-specific persistence for records of type `R`.
///
/// Implemented by each **store** type (e.g. `SqliteRepository`) once per
/// domain model it supports.  The generic [`TaxRepository`] façade
/// delegates to these methods automatically.
///
/// ```ignore
/// // In the backend crate:
/// #[async_trait]
/// impl Persist<TaxBracket> for SqliteRepository { … }
/// ```
#[async_trait]
pub trait Persist<R: TaxRecord>: Send + Sync {
    /// Retrieve a single record by primary key.
    async fn fetch(
        &self,
        key: &R::Key,
    ) -> Result<R, RepositoryError>;

    /// List records matching the given filter.
    async fn fetch_all(
        &self,
        filter: &R::Filter,
    ) -> Result<Vec<R>, RepositoryError>;

    /// Insert a new record from the given draft and return the persisted
    /// record (with any generated fields such as `id` or timestamps).
    async fn create(
        &self,
        draft: R::Draft,
    ) -> Result<R, RepositoryError>;

    /// Persist changes to an existing record.
    ///
    /// Returns a configuration error by default — reference-data models
    /// that are never updated after seeding need not override this.
    async fn update(
        &self,
        record: &R,
    ) -> Result<(), RepositoryError> {
        let _ = record;
        Err(RepositoryError::Configuration(
            "update not supported for this record".into(),
        ))
    }

    /// Delete a single record by primary key.
    async fn delete(
        &self,
        key: &R::Key,
    ) -> Result<(), RepositoryError>;

    /// Delete all records matching the given filter.
    ///
    /// Returns a configuration error by default.
    async fn delete_all(
        &self,
        filter: &R::Filter,
    ) -> Result<u64, RepositoryError> {
        let _ = filter;
        Err(RepositoryError::Configuration(
            "delete_all not supported for this record".into(),
        ))
    }
}
