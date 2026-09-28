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
├── docs/               # Design documents and docs/config.toml
├── .cargo/config.toml  # Linker and profile settings
├── Cargo.toml          # Workspace manifest
├── rust-toolchain.toml # Pins the Rust channel
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

1. `tax-ui` loads `config.toml` (`[database] backend` and `url`, plus logging, recent connections, and window geometry).
2. A repository is created through `RepositoryRegistry` (currently `sqlite` backend).
3. SQLite migrations and embedded seed SQL are applied automatically during repository initialization.
4. UI loads tax-year data (`TaxYearConfig`, filing statuses, standard deductions, tax brackets).
5. User enters worksheet values, calculations run in `tax-core`.
6. Persist flow writes:
   - `create_estimate(TaxEstimateInput)`
   - then `update_estimate(TaxEstimate { computed: Some(TaxEstimateComputed { ... }) })`

## Configuration

`tax-ui` uses a TOML config file. Every section is optional; missing values use the defaults. A relative database `url` is resolved from the current working directory.

```toml
[database]
backend = "sqlite"
url = "taxes.db"

[logging]
level = "info"
application_only = true
stdout = true
file_enabled = false
```

`docs/config.toml` is a full example, including the connection pool, log file path, and recent connections.

Files that still have the former top-level keys `database_backend` and `database_url` are read when `[database]` is absent. The next save writes `[database]` and drops those keys.

Default config location:

- Linux: `$XDG_CONFIG_HOME/TaxEstimator/config.toml` (or `~/.config/TaxEstimator/config.toml`)
- macOS: `~/Library/Application Support/TaxEstimator/config.toml`
- Windows: `%APPDATA%\TaxEstimator\config.toml`

## Logging

Defaults: level `info`, workspace crates only, stdout on, file logging off. When file logging is enabled, the default path is `TaxEstimator.log` in the working directory.

`RUST_LOG` overrides `level` and `application_only`. A full directive such as `RUST_LOG=gpui=debug,tax_ui=trace` is used as written. Preferences can change logging, the connection pool, and the recent-connection limit while the app is running.

## Quick Start

### Prerequisites

- Rust 1.98, pinned in `rust-toolchain.toml`. The workspace uses edition 2024.
- On `x86_64-unknown-linux-gnu`, `.cargo/config.toml` links with `clang` and `mold`. Install both before building.

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

## First Run

If `config.toml` is missing, the app writes the defaults at the platform path above.

If `database.url` names a file that is not there, startup asks you to open an existing database, create a new one, or quit. `:memory:` opens with no file.

Opening a database applies migrations, then the seeds embedded in `tax-db-sqlite`: filing statuses, and 2025 and 2026 year config, standard deductions, and tax brackets. A normal launch does not need the CSV loader.

## Using the App

Required inputs are tax year, filing status, expected AGI, and expected deduction. Optional inputs are the QBI deduction, AMT, credits, other taxes, withholding, prior-year tax, and the self-employment worksheet (SE income, Conservation Reserve Program payments, and wages subject to Social Security tax).

The deduction field stays empty until you fill it. Seeded standard-deduction amounts are reference data. The SE worksheet shows the deductible half of self-employment tax; include that reduction yourself in expected AGI.

Calculate runs the worksheets and saves on its own: `create_estimate`, then `update_estimate` with the computed summary. Another save for the same tax year and filing status updates that row.

The results panel shows self-employment tax, total tax, and the required annual payment.

The File menu can create, open, save, and save-as a database connection, reopen a recent connection, and load a saved estimate. Load Estimate reads rows already stored in the database.

Window size and position are restored from `[window_geometry]`.

## Loading Tax Brackets from CSV

Use this when you want to replace bracket rows. The desktop app already seeds 2025 and 2026.

```bash
cargo run -p tax-data --bin tax-data-loader -- \
  --file tax-data/test-data/tax_brackets_2025.csv \
  --database taxes.db \
  --migrate \
  --seeds tax-db-sqlite/seeds
```

`--database` is a filesystem path, or `:memory:`. `--migrate` applies the embedded schema. `--seeds` applies the seeds compiled into `tax-db-sqlite`; the path value is required by the flag and is not read. The loader then deletes and reinserts brackets for each tax year and schedule in the CSV.

CSV schedule mappings:

- `X` -> `S`
- `Y-1` -> `MFJ` and `QSS`
- `Y-2` -> `MFS`
- `Z` -> `HOH`

## Database Notes

- Schema migration lives in `tax-db-sqlite/migrations/`.
- Seed SQL lives in `tax-db-sqlite/seeds/` and is embedded into the binary at build time.
- `tax_estimate` has one row per `(tax_year, filing_status_id)`. `create_estimate` upserts on that unique index.
- In-memory mode (`:memory:`) is supported for tests.

## Known Limitations (Current Behavior)

- Additional context values in estimated-tax calculation are currently fixed in the UI:
  - `refundable_credits = 0`
  - `is_farmer_or_fisher = false`
- Safe-harbor `110%` prior-year logic is not auto-derived; the caller provides the prior-year tax already adjusted.
- QBI and AMT are entered amounts. Form 8995 is field help, not a calculator.
- Additional Medicare Tax / NIIT are not modeled as dedicated calculators (enter an estimate in "other taxes").
- The standard deduction is not copied into the deduction field, and the deductible half of SE tax is not subtracted from AGI.
- Quarterly due dates are out of scope. The worksheet computes an underpayment figure; the results panel and the saved summary store self-employment tax, total tax, and the required annual payment.

## Docs

Design documents and the example config are in `docs/`:

- `docs/config.toml`
- `docs/TaxEstimatePersistencePlan.md`
- `docs/BackendPersistenceMigration.md`
- `docs/WASM_Plan.md`
- `docs/SeWorksheet.md`

## License

MIT
