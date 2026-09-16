use std::sync::Arc;

use anyhow::Result;
use tax_core::TaxRepository;
use tax_core::db::DbConfig;

use crate::repository::backends::build_registry;

/// Opens (or creates) a repository for `db_config` using the backend registry.
pub async fn open_repository(db_config: &DbConfig) -> Result<Arc<dyn TaxRepository>> {
    let registry = build_registry();
    let repo = registry.create(db_config).await?;
    Ok(repo.into())
}
