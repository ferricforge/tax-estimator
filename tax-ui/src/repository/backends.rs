//! Backend registration. Every database backend this build can open is listed
//! here exactly once.

use tax_core::db::RepositoryRegistry;
use tax_db_sqlite::SqliteRepositoryFactory;

/// Registers every known backend with a fresh [`RepositoryRegistry`].
///
/// Adding a backend later is one `register` call.
pub fn build_registry() -> RepositoryRegistry {
    let mut registry = RepositoryRegistry::new();
    registry.register(Box::new(SqliteRepositoryFactory));
    registry
}
