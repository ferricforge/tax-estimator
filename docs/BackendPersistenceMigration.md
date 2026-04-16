# Backend Persistence Migration

## Goal

Use `main` as the clean starting point for a backend-owned persistence design.

The target architecture is:

- `tax-core` owns domain models and domain-facing repository traits.
- `tax-db-sqlite` owns SQLite row models, SQL, and mapping code.
- a future `tax-db-mysql` would own its own row models and mapping code, following the same pattern.
- repository methods stay domain-oriented instead of becoming a generic table CRUD layer.

This document assumes the current `main` branch, where persistence is still implemented with hand-written SQLx code in `tax-db-sqlite`.

## Current Baseline on `main`

Today the codebase already has the right high-level split:

- `tax-core` defines domain models.
- `tax-core` defines the `TaxRepository` trait.
- `tax-db-sqlite` implements that trait.

What is still mixed together is the persistence boundary inside `tax-db-sqlite`:

- repository methods query raw SQL
- decode directly from `SqliteRow`
- assemble domain models inline
- bind domain model fields directly into SQL writes

That works, but it makes the repository implementation responsible for too many concerns at once:

- query logic
- row decoding
- storage-to-domain conversion
- domain-to-storage conversion

The migration in this document is about introducing an explicit storage layer inside backend crates, not about changing the domain trait surface first.

## Core Decision

Backend crates should introduce backend-specific row models and convert between those models and `tax-core` domain types.

That means:

- `tax-core` models stay storage-agnostic
- backend crates own persisted row structs
- backend crates implement `From`/`TryFrom` where conversion is local and self-contained
- repository methods orchestrate queries and conversions, but stop constructing domain values field-by-field inline

## Design Principles

### 1. Domain models are not storage models

`tax-core` should define:

- domain structs
- validation rules
- calculation logic
- repository traits expressed in domain terms

`tax-core` should not define:

- table names
- SQL strings
- SQLx row decoding helpers
- backend-specific attributes
- database-only field shapes

### 2. Backends own persisted shape

Each backend crate should define the storage representation that best matches its schema and query layer.

That includes:

- row structs
- write structs where useful
- enum/string/int translation details
- decimal translation details
- join result translation

The important boundary is:

- domain types are shared
- persisted row types are backend-owned

### 3. Repository traits remain domain-oriented

The `TaxRepository` trait in `tax-core` should continue describing domain operations like:

- `get_tax_year_config`
- `get_standard_deduction`
- `get_tax_brackets`
- `delete_tax_brackets`
- `create_estimate`
- `update_estimate`

Do not try to force the whole repository into `save/get/delete/list` just because some row types are table-shaped.

### 4. Use `From` and `TryFrom` selectively

Use `From` when:

- conversion is structural
- conversion is lossless
- no I/O or lookup is required

Use `TryFrom` when:

- storage contents may be invalid
- enum parsing can fail
- decimal conversion can fail
- nullability or partial row state needs validation

Do not force `From` or `TryFrom` when:

- conversion depends on repository lookups
- a domain object is assembled from multiple joined rows
- the repository method is really about aggregate semantics rather than a single row

## Target Module Layout

### `tax-core`

Keep `tax-core` focused on domain code.

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

No backend row models should live here.

### `tax-db-sqlite`

Add a backend-owned models module.

Recommended first-step shape:

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

If the mapping layer grows, a later split can be:

```text
tax-db-sqlite/
  src/
    models/
      mod.rs
      rows/
      mappers/
```

That split is optional. The important part is simply getting row types out of `repository.rs`.

### Future `tax-db-mysql`

Mirror the same boundary:

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

Do not share row structs between SQLite and MySQL. The backend crates should share domain types, not persisted shapes.

## Row Model Strategy by Domain Type

### `FilingStatus`

Good fit for a backend row model.

Recommended:

- `FilingStatusRow`
- `impl From<&tax_core::FilingStatus> for FilingStatusRow`
- `impl TryFrom<FilingStatusRow> for tax_core::FilingStatus`

Why:

- database contents might contain an invalid status code
- that makes read conversion fallible

### `TaxYearConfig`

Good fit for a backend row model.

Recommended:

- `TaxYearConfigRow`
- `impl From<&tax_core::TaxYearConfig> for TaxYearConfigRow`
- `impl TryFrom<TaxYearConfigRow> for tax_core::TaxYearConfig`

Why:

- it is a simple single-row, single-table domain type

### `StandardDeduction`

Good fit for a backend row model.

Recommended:

- `StandardDeductionRow`
- `impl From<&tax_core::StandardDeduction> for StandardDeductionRow`
- `impl TryFrom<StandardDeductionRow> for tax_core::StandardDeduction`

Why:

- it is simple
- it maps directly to one table row

### `TaxBracket`

Use a backend row model, but keep repository semantics explicit.

Recommended:

- `TaxBracketRow`
- `impl From<&tax_core::TaxBracket> for TaxBracketRow`
- `impl TryFrom<TaxBracketRow> for tax_core::TaxBracket`

Important caveat:

- row mapping is fine
- repository semantics are not purely row CRUD

The current contract includes:

- `get_tax_brackets(tax_year, filing_status_id)`
- `delete_tax_brackets(tax_year, filing_status_id)`

That is a grouped operation, not a single-row delete API, and the migration should preserve that.

### `TaxEstimate`

Use backend row models, but do not force the same pattern as the simpler types.

Why `TaxEstimate` is different:

- the domain object is nested
- stored values are flattened
- the domain uses `FilingStatusCode`
- storage uses a filing status foreign key and joins to reconstruct the code
- computed values are grouped under `TaxEstimateComputed`

Recommended approach:

- introduce a `TaxEstimateRow` for the flattened read shape
- optionally introduce a separate write struct if that makes create/update logic clearer
- use explicit mapping helpers for read and write conversion

For example:

- `fn try_into_domain(self) -> Result<TaxEstimate, RepositoryError>`
- `fn from_domain_input(...) -> Result<TaxEstimateWriteRow, RepositoryError>`
- `fn from_domain_estimate(...) -> Result<TaxEstimateWriteRow, RepositoryError>`

This is clearer than trying to make every `TaxEstimate` conversion fit a single blanket trait pattern.

## Example Pattern

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

1. querying backend rows
2. mapping backend rows into domain models
3. mapping domain models into backend write rows before writes

That is the right level of responsibility for the repository implementation.

## Repository Refactor Shape

### Current pattern on `main`

Repository methods often do this directly:

1. run SQL
2. pull each field out of `SqliteRow`
3. assemble the domain model inline

For writes they often:

1. take a domain model
2. bind domain fields directly into SQL
3. execute

### Target pattern

Repository methods should move to:

1. run SQL
2. decode into a backend row model
3. convert the backend row model into a domain model
4. return the domain model

For writes:

1. convert the domain model into a backend row or write struct
2. bind storage values from that backend type
3. execute

That change makes the storage boundary explicit and testable without changing the domain repository API.

## Migration Phases from `main`

### Phase 1: Introduce backend row modules with no behavior change

Add `tax-db-sqlite/src/models/` and create row structs for:

- `FilingStatus`
- `StandardDeduction`
- `TaxBracket`
- `TaxYearConfig`
- `TaxEstimate`

In this phase:

- do not change `TaxRepository`
- do not change SQL queries yet
- do not try to redesign the domain model
- just create the backend-owned types and mapping helpers

Success criteria:

- repository behavior stays identical
- repository methods get thinner over time

### Phase 2: Move read mapping out of `repository.rs`

Refactor the simplest reads first:

- `get_tax_year_config`
- `get_filing_status`
- `get_filing_status_by_code`
- `list_filing_statuses`
- `get_standard_deduction`
- `get_tax_brackets`

Why these first:

- they are straightforward
- they exercise the row-to-domain conversion layer
- they reduce the amount of inline row parsing in the repository quickly

### Phase 3: Move simple write mapping out of `repository.rs`

Next refactor:

- `insert_tax_bracket`

If `StandardDeduction` gains a write path again later, handle it the same way:

- domain model in
- backend write row out
- bind storage values from the backend write type

### Phase 4: Refactor `TaxEstimate` with explicit mapping helpers

Handle `TaxEstimate` separately.

Keep the domain repository surface explicit:

- `create_estimate`
- `get_estimate`
- `update_estimate`
- `delete_estimate`
- `list_estimates`

Do not try to flatten this into a generic row CRUD abstraction.

The right goal here is:

- clearer read mapping
- clearer write mapping
- repository methods that are easier to reason about

### Phase 5: Evaluate whether any generic persistence helper is still needed

Starting from `main`, there is no need to introduce a proc-macro or generic entity layer first.

After the backend row model split is in place, reassess:

- whether a small helper trait would reduce duplication
- whether duplication is low enough that explicit code is better

The default recommendation is to stay explicit unless a pattern is truly repetitive and stable.

## File Ownership Rules

### `tax-core`

Owns:

- domain models
- repository traits
- repository error types
- calculation logic

Should avoid:

- backend storage metadata
- backend row structs
- SQL binding concerns

### `tax-db-sqlite`

Owns:

- SQLite row structs
- SQLite-to-domain conversions
- domain-to-SQLite write conversions
- SQLx query details
- SQLx decoding details

### Future `tax-db-mysql`

Owns:

- MySQL row structs
- MySQL conversion details
- MySQL-specific query behavior

## Testing Strategy

### Unit tests for row conversion

Add focused tests for backend row models:

- valid `FilingStatusRow -> FilingStatus`
- invalid filing status code handling
- decimal conversion edge cases
- partial computed field validation for `TaxEstimate`

### Repository tests remain domain-facing

Existing repository tests should continue asserting:

- repository methods return domain models
- grouped operations like `delete_tax_brackets` keep their current semantics
- `TaxEstimate` behavior remains unchanged

### Migration guardrail

Do not remove any inline mapping path until the equivalent backend row mapping is covered by tests and used successfully by repository methods.

## What Not To Do

- Do not redesign `TaxRepository` around generic CRUD as a first step.
- Do not put backend row structs into `tax-core`.
- Do not share SQLite row structs with a future MySQL backend.
- Do not force `TaxEstimate` to look like a simple table row abstraction.
- Do not introduce a proc-macro layer unless explicit backend mapping proves too repetitive later.

## Decision Summary

Starting from `main`, the recommended path is:

- keep `tax-core` as the domain layer
- introduce backend-owned row models in `tax-db-sqlite`
- use `From` and `TryFrom` for simple local conversions
- use explicit mapping helpers for `TaxEstimate` and other richer cases
- keep repository traits domain-oriented

That gives you a clean foundation for starting over without bringing the proc-macro experiment forward into the new design.
