//! SQLite-backed implementation of [`tax_core::TaxRepository`].
//!
//! The implementation is split into private submodules: the `SqliteRepository`
//! type and its lifecycle (pool, migrations, seeds, backup); the
//! `TaxRepository` query orchestration; and the backend-owned row structs with
//! their fallible conversions into the `tax_core` domain models.

mod rows;
mod sqlite_repository;
mod tax_repository;
#[cfg(test)]
mod tests;

pub use sqlite_repository::SqliteRepository;
