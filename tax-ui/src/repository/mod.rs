//! Access to the tax database: backend registration, opening connections, and
//! the process-wide repository handle.
//!
//! Nothing in here reads application settings or cached reference data.
//! Combining those with a connection is the job of [`crate::session`].

mod backends;
mod connect;
mod handle;

pub use backends::build_registry;
pub use connect::open_repository;
pub use handle::{RepositoryUnavailable, TaxRepo};
