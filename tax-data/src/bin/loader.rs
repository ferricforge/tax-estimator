use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tax_data::TaxBracketLoader;
use tax_db_sqlite::SqliteRepository;

/// Load tax bracket data from a CSV file into the database.
///
/// The CSV file should have the following columns:
/// - tax_year: The tax year (e.g., 2025)
/// - schedule: The IRS schedule code (X, Y-1, Y-2, Z)
/// - min_income: The minimum income for this bracket
/// - max_income: The maximum income (empty for unlimited)
/// - base_tax: The base tax amount for this bracket
/// - rate: The marginal tax rate as a decimal (e.g., 0.10)
#[derive(Parser, Debug)]
#[command(name = "tax-data-loader")]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the CSV file containing tax bracket data
    #[arg(short, long)]
    file: PathBuf,

    /// SQLite database URL (e.g., sqlite:tax.db?mode=rwc to create if missing)
    #[arg(short, long, default_value = "sqlite:tax.db?mode=rwc")]
    database: String,

    /// Run database migrations before loading data
    #[arg(short, long, default_value_t = false)]
    migrate: bool,

    /// Run seed files from the specified directory after migrations
    #[arg(short, long)]
    seeds: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let repo = SqliteRepository::new(&args.database)
        .await
        .with_context(|| format!("Failed to connect to database: {}", args.database))?;

    if args.migrate {
        println!("Running migrations...");
        repo.run_migrations()
            .await
            .context("Failed to run migrations")?;
        println!("Migrations complete.");
    }

    if let Some(seeds_dir) = &args.seeds {
        println!("Running seeds from: {}", seeds_dir.display());
        repo.run_seeds(seeds_dir)
            .await
            .with_context(|| format!("Failed to run seeds from: {}", seeds_dir.display()))?;
        println!("Seeds complete.");
    }

    println!("Loading tax brackets from: {}", args.file.display());

    let file = File::open(&args.file)
        .with_context(|| format!("Failed to open: {}", args.file.display()))?;

    let records = TaxBracketLoader::parse(file)
        .with_context(|| format!("Failed to parse CSV: {}", args.file.display()))?;

    println!("Parsed {} records from CSV", records.len());

    let inserted = TaxBracketLoader::load(&repo, &records)
        .await
        .context("Failed to load tax brackets into database")?;

    println!(
        "Successfully loaded {} tax brackets into the database.",
        inserted
    );

    Ok(())
}
