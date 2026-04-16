# Backend Persistence Migration

## Goal

Move persistence details out of `tax-core` domain models and into backend crates such as `tax-db-sqlite`, with a layout that also works for a future `tax-db-mysql`.

The target outcome is:

- `tax-core` owns domain models and domain-facing repository traits.
- Each backend crate owns its own persisted row models.
- Backend crates convert between persisted rows and domain models with `From`/`TryFrom` where that conversion is self-contained.
- Repository methods remain domain-oriented instead of collapsing into a universal table CRUD abstraction.

This document is written against the `features/proc_macros` branch as a migration starting point.

## Why Change

The current experiment proves that auto-generated persistence can work for simple row-shaped types, but it also exposes the main boundary problem:

- `tax-core` models currently participate in persistence concerns.
- The `Entity` derive generates SQLx/SQLite-centric methods on domain models.
- A generic row CRUD abstraction fits `StandardDeduction` better than it fits richer types like `TaxEstimate`.
- Collection and aggregate semantics already exist in the repository surface, especially around `TaxBracket` and `TaxEstimate`.

The architectural issue is not that row-model automation is always wrong. It is that the automation currently lives on the wrong side of the boundary.

## Migration Principles

### 1. Domain models stay backend-agnostic

`tax-core` should define:

- domain structs
- validation
- calculation logic
- repository traits expressed in domain terms

`tax-core` should not define:

- table names
- SQL strings
- SQLx row decoding
- backend-specific `insert/find/delete` helpers

### 2. Backends own storage shape

Each backend crate should define the storage representation it needs. That includes:

- row structs
- SQL
- joins
- backend-specific IDs or foreign key representations where necessary
- conversion and mapping code

### 3. Use `From`/`TryFrom` only for pure shape conversion

Use `From` when:

- conversion is lossless
- no database lookup is needed
- no runtime validation beyond structural conversion is needed

Use `TryFrom` when:

- database values may be invalid
- decoding may fail due to enum parsing, decimal conversion, nullability, or partial row state

Do not force `From`/`TryFrom` when:

- conversion depends on repository lookups
- conversion spans multiple joined tables
- conversion is really an aggregate assembly step rather than a row mapping step

### 4. Repository traits remain domain-oriented

Keep domain operations explicit even if some of them are implemented with backend row models under the hood.

Good repository operations:

- `get_tax_year_config(year)`
- `get_standard_deduction(year, filing_status_id)`
- `get_tax_brackets(year, filing_status_id)`
- `delete_tax_brackets(year, filing_status_id)`
- `create_estimate(input)`
- `update_estimate(&estimate)`

Avoid making the repository API pretend that everything is generic row CRUD when the domain does not behave that way.

## Target Module Layout

### `tax-core`

No backend row types live here.

Recommended shape:

```text
tax-core/
  src/
    calculations/
    db/
      factory.rs
      mod.rs
      repository.rs
    models/
      filing_status.rs
      standard_deduction.rs
      tax_bracket.rs
      tax_estimate.rs
      tax_year_config.rs
```

Notes:

- `tax-core::db::repository` remains the home of `TaxRepository`.
- If the proc-macro experiment is retired, `tax-core` should eventually stop depending on `tax-db-macros`.

### `tax-db-sqlite`

Add a backend-owned `models` module for persisted row shapes and mapping helpers.

Recommended shape:

```text
tax-db-sqlite/
  src/
    decimal.rs
    factory.rs
    lib.rs
    models/
      mod.rs
      filing_status_row.rs
      standard_deduction_row.rs
      tax_bracket_row.rs
      tax_estimate_row.rs
      tax_year_config_row.rs
    repository.rs
```

Optional later refinement:

```text
tax-db-sqlite/
  src/
    models/
      mod.rs
      rows/
      mappers/
```

That split is only worth it once the number of row structs and mapping helpers grows enough to justify the extra layer.

### Future `tax-db-mysql`

Mirror the same idea without sharing persisted row structs across backends.

Recommended shape:

```text
tax-db-mysql/
  src/
    factory.rs
    lib.rs
    models/
      mod.rs
      filing_status_row.rs
      standard_deduction_row.rs
      tax_bracket_row.rs
      tax_estimate_row.rs
      tax_year_config_row.rs
    repository.rs
```

The domain type is shared. The persisted row shape is not.

## Row Model Responsibilities

### Simple reference data

These domain types are good fits for backend row structs plus `From`/`TryFrom`:

- `StandardDeduction`
- `TaxYearConfig`
- `FilingStatus`
- `TaxBracket`

Each backend row type should:

- match the backend schema directly
- represent values in the form most natural to the backend query layer
- convert into the domain type

For example, a SQLite row model may store decimal-backed fields in the shape that best matches the SQLx query output, while the domain model stays on `Decimal`.

### Aggregate and workflow data

`TaxEstimate` should still use backend row models, but it should not be forced into the same pattern as the simpler types.

Why:

- the domain model is nested
- `filing_status_id` in storage becomes `FilingStatusCode` in the domain type
- the computed payload is grouped into `TaxEstimateComputed`
- some conversions are only valid after additional interpretation

That makes `TaxEstimate` a better fit for explicit backend mapping helpers than a pure mechanical row conversion everywhere.

## Recommended Conversion Rules by Type

### `StandardDeduction`

Recommended:

- `impl From<&tax_core::StandardDeduction> for StandardDeductionRow`
- `impl TryFrom<StandardDeductionRow> for tax_core::StandardDeduction`

Rationale:

- write path is structural
- read path may need decimal conversion safety

### `TaxYearConfig`

Recommended:

- `impl From<&tax_core::TaxYearConfig> for TaxYearConfigRow`
- `impl TryFrom<TaxYearConfigRow> for tax_core::TaxYearConfig`

Rationale:

- still a single-table, row-shaped domain type

### `FilingStatus`

Recommended:

- `impl From<&tax_core::FilingStatus> for FilingStatusRow`
- `impl TryFrom<FilingStatusRow> for tax_core::FilingStatus`

Rationale:

- `status_code` parsing can fail if the database contents drift from expectations

### `TaxBracket`

Recommended:

- `impl From<&tax_core::TaxBracket> for TaxBracketRow`
- `impl TryFrom<TaxBracketRow> for tax_core::TaxBracket`

Important caveat:

- row conversion is fine
- repository semantics should still keep collection operations explicit

Do not replace:

- `delete_tax_brackets(year, filing_status_id)`

with:

- `delete::<TaxBracket>(key)`

unless the domain contract is intentionally changed.

### `TaxEstimate`

Recommended:

- backend row struct for the flattened persisted row
- explicit mapping helpers instead of insisting on `From` everywhere

Possible shape:

```rust
struct TaxEstimateRow {
    id: i64,
    tax_year: i32,
    filing_status_code: String,
    expected_agi: f64,
    expected_deduction: f64,
    expected_qbi_deduction: Option<f64>,
    expected_amt: Option<f64>,
    expected_credits: Option<f64>,
    expected_other_taxes: Option<f64>,
    expected_withholding: Option<f64>,
    prior_year_tax: Option<f64>,
    se_income: Option<f64>,
    expected_crp_payments: Option<f64>,
    expected_wages: Option<f64>,
    calculated_se_tax: Option<f64>,
    calculated_total_tax: Option<f64>,
    calculated_required_payment: Option<f64>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}
```

Recommended mapping functions:

- `fn try_into_domain(self) -> Result<TaxEstimate, RepositoryError>`
- `fn from_domain_input(...) -> Result<TaxEstimateWriteRow, RepositoryError>`
- `fn from_domain_estimate(...) -> Result<TaxEstimateWriteRow, RepositoryError>`

This keeps the complicated mapping explicit and readable.

## Example Backend Model Pattern

Illustrative shape only:

```rust
pub struct StandardDeductionRow {
    pub tax_year: i32,
    pub filing_status_id: i32,
    pub amount: f64,
}

impl From<&tax_core::StandardDeduction> for StandardDeductionRow {
    fn from(value: &tax_core::StandardDeduction) -> Self {
        Self {
            tax_year: value.tax_year,
            filing_status_id: value.filing_status_id,
            amount: crate::decimal::decimal_to_f64(value.amount),
        }
    }
}

impl TryFrom<StandardDeductionRow> for tax_core::StandardDeduction {
    type Error = tax_core::RepositoryError;

    fn try_from(row: StandardDeductionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            amount: rust_decimal::Decimal::try_from(row.amount).map_err(|e| {
                tax_core::RepositoryError::InvalidData(format!(
                    "invalid standard deduction amount: {e}"
                ))
            })?,
        })
    }
}
```

The repository then becomes responsible for:

- querying backend rows
- converting backend rows to domain models
- converting domain models to backend rows before writes

## Repository Refactor Shape

The repository implementation should progressively move from inline row assembly to backend row models.

### Current pattern

Repository methods often do this directly:

- query raw SQL
- pull each field out of `SqliteRow`
- construct the domain model inline

### Target pattern

Repository methods should instead do:

1. query
2. decode into backend row model
3. convert backend row model into domain model
4. return the domain model

For writes:

1. convert domain model into backend write row
2. bind row values
3. execute SQL

This makes the persistence boundary visible and testable.

## Recommended Rollout Phases

### Phase 1: Introduce backend row modules without behavior change

On `features/proc_macros`, add the `tax-db-sqlite/src/models/` module and backend row structs for:

- `FilingStatus`
- `StandardDeduction`
- `TaxBracket`
- `TaxYearConfig`
- `TaxEstimate`

In this phase:

- keep the current repository trait unchanged
- keep query SQL unchanged
- only move row decoding and mapping into backend models/helpers

Success criteria:

- all existing tests still pass
- repository methods become thinner

### Phase 2: Refactor simple repository methods to use row models

Start with:

- `get_tax_year_config`
- `get_filing_status`
- `get_filing_status_by_code`
- `list_filing_statuses`
- `get_standard_deduction`
- `get_tax_brackets`

These are the best first candidates because they are read-oriented and straightforward.

### Phase 3: Refactor write paths for simple row-shaped types

Next move:

- `insert_standard_deduction`
- `insert_tax_bracket`

For these methods, convert the domain model into backend row models before binding values.

This establishes the write-side pattern without changing repository semantics.

### Phase 4: Refactor `TaxEstimate` using explicit backend mapping

Move `TaxEstimate` off inline row construction and into dedicated backend mapping helpers.

Do not force a generic CRUD shape here.

Keep:

- `create_estimate`
- `get_estimate`
- `update_estimate`
- `delete_estimate`
- `list_estimates`

as explicit domain operations.

### Phase 5: Retire proc-macro persistence from `tax-core`

After backend row models fully own persistence concerns:

- remove `tax-db-macros` usage from domain structs
- remove generated persistence methods from domain types
- remove any generic entity abstraction that no longer serves a clear domain purpose

This is the point where `tax-core` becomes cleanly backend-neutral again.

## Suggested File Ownership During Migration

### `tax-core`

Allowed changes:

- repository trait adjustments only if they are domain-motivated
- error types
- domain models only for domain reasons

Avoid:

- adding backend row metadata
- adding table names
- adding storage attributes

### `tax-db-sqlite`

Owns:

- row structs
- `TryFrom`/`From` conversions for backend rows
- SQL
- SQLx result decoding
- backend mapping helpers

### Future `tax-db-mysql`

Will own:

- different row structs where needed
- MySQL-specific conversion details
- its own repository implementation that targets the same domain trait

## Testing Strategy

### Unit tests for backend row conversions

Add focused tests for:

- valid `FilingStatusRow -> FilingStatus`
- invalid status code handling
- decimal conversion edge cases
- partial computed field handling for `TaxEstimate`

### Repository tests stay domain-facing

Keep repository integration tests asserting domain behavior:

- repository methods return domain models
- deletes preserve current semantics
- `TaxEstimate` behavior remains unchanged

### Migration guardrail

Before deleting any proc-macro-based persistence code, make sure all repository tests pass without relying on domain-model SQL helpers.

## What Not To Do

- Do not replace the whole repository trait with generic `save/get/delete/list` methods.
- Do not share backend row structs between SQLite and MySQL.
- Do not move domain validation into backend row models.
- Do not require `TaxEstimate` to fit the same conversion pattern as `StandardDeduction`.
- Do not use `Default::default()` to silently fabricate skipped domain fields during read mapping.

## Decision Summary

The recommended migration is:

- backend row models live in backend crates
- domain models stay in `tax-core`
- `From`/`TryFrom` is used for simple, local conversions
- explicit mapping helpers are used where conversion requires interpretation or grouping
- repository traits stay domain-oriented

This keeps the project open to future backends without making the domain layer carry SQLite-specific behavior.
