use std::env;

use tax_core::db::{DbConfig, RepositoryRegistry};
use tax_db_sqlite::SqliteRepositoryFactory;

/// Assemble a [`RepositoryRegistry`] with every known backend.
/// Adding a new backend in the future is a single `register` call here.
fn build_registry() -> RepositoryRegistry {
    let mut registry = RepositoryRegistry::new();
    registry.register(Box::new(SqliteRepositoryFactory));
    registry
}

/// Derive a [`DbConfig`] from positional CLI arguments with sensible
/// defaults for local development.
///
/// Usage:
///   tax-ui                          # sqlite, taxes.db
///   tax-ui sqlite taxes.db          # explicit, same effect
///   tax-ui sqlite :memory:          # ephemeral in-memory DB
fn config_from_args() -> DbConfig {
    let args: Vec<String> = env::args().collect();
    DbConfig {
        backend: args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "sqlite".to_string()),
        connection_string: args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "taxes.db".to_string()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config_from_args();
    eprintln!(
        "backend={}, connection={}",
        config.backend, config.connection_string
    );

    let registry = build_registry();
    let repo = registry.create(&config).await?;

    // ── smoke check: list the tax years currently in the database ────
    let years = repo.list_tax_years().await?;
    if years.is_empty() {
        eprintln!("Warning: no tax-year data found — run the seeder first.");
    } else {
        println!("Available tax years: {:?}", years);
    }

    Ok(())
}
