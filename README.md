# Tax Estimator

Rust application for calculating U.S. federal estimated tax using IRS Form 1040-ES worksheets.

## What This Project Is

This application is a multi-crate Rust workspace with:

- A **desktop UI** built with GPUI (`tax-ui`)
- A **domain + calculation layer** (`tax-core`)
- A **SQLite backend** reference implementation of the repository trait (`tax-db-sqlite`)
- A **CSV data-loading utility** for tax brackets (`tax-data`)

The app currently supports:

- SE Tax and Deduction Worksheet calculations
- Estimated Tax Worksheet calculations (including filing-status-specific tax brackets)
- Persisting estimate inputs and computed results to SQLite
- Filing statuses: `S`, `MFJ`, `MFS`, `HOH`, `QSS`

## Workspace Layout

```text
tax-estimator/
├── tax-core/           # Domain models, repository interfaces, worksheet calculations
├── tax-db-sqlite/      # SQLx/SQLite repository implementation + migrations + seed SQL
├── tax-data/           # CSV-to-database loader CLI for tax bracket schedules
├── tax-ui/             # GPUI desktop application
├── docs/               # Design/roadmap documents
├── Cargo.toml          # Workspace manifest
└── rustfmt.toml
```

## Crates

| Crate | Purpose |
|---|---|
| `tax-core` | Core domain models (`TaxEstimateInput`, `TaxEstimate`, `TaxYearConfig`, etc.), repository traits, and worksheet calculation engines |
| `tax-db-sqlite` | `TaxRepository` reference implementation using SQLite + SQLx migrations/seeds |
| `tax-data` | CLI for loading IRS tax bracket CSV data into a repository-backed database |
| `tax-ui` | Desktop UI that loads tax-year data, computes worksheet values, and saves estimates |

## Runtime Architecture

1. `tax-ui` initializes app configuration (`database_backend`, `database_url`).
2. A repository is created through `RepositoryRegistry` (currently `sqlite` backend).
3. SQLite migrations and seed SQL are applied automatically during repository initialization.
4. UI loads tax-year data (`TaxYearConfig`, filing statuses, standard deductions, tax brackets).
5. User enters worksheet values, calculations run in `tax-core`.
6. Persist flow writes:
   - `create_estimate(TaxEstimateInput)`
   - then `update_estimate(TaxEstimate { computed: Some(TaxEstimateComputed { ... }) })`

## Configuration

`tax-ui` uses a TOML config file. Defaults:

- `database_backend = "sqlite"`
- `database_url = "taxes.db"` (in current working directory if relative)

Default config location:

- Linux: `$XDG_CONFIG_HOME/TaxEstimator/config.toml` (or `~/.config/TaxEstimator/config.toml`)
- macOS: `~/Library/Application Support/TaxEstimator/config.toml`
- Windows: `%APPDATA%\TaxEstimator\config.toml`

## Quick Start

### Prerequisites

- Rust toolchain (edition 2024 workspace)

### Build

```bash
cargo build --workspace
```

### Run the desktop app

```bash
cargo run -p tax-ui --bin TaxEstimator
```

### Run tests

```bash
cargo test --workspace
```

## Loading Tax Brackets from CSV

The `tax-data` crate provides a loader CLI:

```bash
cargo run -p tax-data --bin tax-data-loader -- \
  --file tax-data/test-data/tax_brackets_2025.csv \
  --database taxes.db \
  --migrate \
  --seeds tax-db-sqlite/seeds
```

CSV schedule mappings:

- `X` -> `S`
- `Y-1` -> `MFJ` and `QSS`
- `Y-2` -> `MFS`
- `Z` -> `HOH`

## Database Notes

- Schema migration lives in `tax-db-sqlite/migrations/`.
- Seed SQL lives in `tax-db-sqlite/seeds/`.
- `tax_estimate` enforces one record per `(tax_year, filing_status_id)` via unique index.
- In-memory mode (`:memory:`) is supported for tests.
- Seed directory resolution can be overridden with `TAX_DB_SQLITE_SEEDS_DIR`.

## Known Limitations (Current Behavior)

- Additional context values in estimated-tax calculation are currently fixed in UI:
  - `refundable_credits = 0`
  - `is_farmer_or_fisher = false`
- Safe-harbor `110%` prior-year logic is not auto-derived; caller provides prior-year value.
- Additional Medicare Tax / NIIT are not modeled as dedicated calculators (can be entered via "other taxes" input as an estimate).
- Quarterly due-date/payment scheduling is out of scope (this app computes annual required payment and underpayment signals).

## Docs

Design and roadmap documents are in `docs/`, including:

- `docs/TaxEstimatePersistencePlan.md`
- `docs/BackendPersistenceMigration.md`
- `docs/WASM_Plan.md`
- `docs/SeWorksheet.md`

## License

MIT
