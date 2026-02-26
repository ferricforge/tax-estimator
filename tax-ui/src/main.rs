use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

use tax_core::db::DbConfig;
use tax_ui::app;

// ─── CLI definition ──────────────────────────────────────────────────────────

/// Estimated tax calculator for IRS Form 1040-ES.
///
/// Connects to the configured database, loads reference data for the
/// requested tax year, and prints it.
#[derive(Debug, Parser)]
struct Cli {
    /// Database backend to use.
    #[arg(long, default_value = "sqlite")]
    backend: String,

    /// Database connection string.
    /// For SQLite this is a file path (e.g. `taxes.db`) or `:memory:`.
    #[arg(long, default_value = "taxes.db")]
    db: String,

    /// Tax year to retrieve and display.
    #[arg(long, default_value = "2025")]
    year: i32,
}

// ─── tracing ─────────────────────────────────────────────────────────────────

/// Initialise the tracing subscriber.
///
/// * Honours `RUST_LOG` when set.
/// * Falls back to `info` so normal runs are quiet.
/// * Strips timestamps and target names to keep CLI output clean.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_target(false)
        .init();
}

// ─── entry point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();

    let db_config = DbConfig {
        backend: cli.backend,
        connection_string: cli.db,
    };

    debug!("connecting to {} backend", db_config.backend);
    let registry = app::build_registry();
    let repo = registry.create(&db_config).await?;

    let data = app::load_tax_year_data(&*repo, cli.year).await?;
    info!("{}", data);

    Ok(())
}
