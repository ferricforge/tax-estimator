//! Backend-owned row structs — one per query shape — decoded with SQLx's
//! [`FromRow`](sqlx::FromRow) and converted into the `tax_core` domain models
//! through fallible [`TryFrom`] impls.

mod estimate;
mod filing_status;
mod filing_status_data;
mod standard_deduction;
mod tax_bracket;
mod tax_year_config;

pub(crate) use estimate::EstimateRow;
pub(crate) use filing_status::FilingStatusRow;
pub(crate) use filing_status_data::FilingStatusDataRow;
pub(crate) use standard_deduction::StandardDeductionRow;
pub(crate) use tax_bracket::TaxBracketRow;
pub(crate) use tax_year_config::TaxYearConfigRow;

use sqlx::{Row, sqlite::SqliteRow};
use tax_core::{FilingStatusCode, RepositoryError};

/// Decode a non-decimal column, mapping decode failures to
/// [`RepositoryError::Database`].
fn field<'r, T>(
    row: &'r SqliteRow,
    column: &str,
) -> Result<T, RepositoryError>
where
    T: sqlx::Decode<'r, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>,
{
    row.try_get::<T, _>(column)
        .map_err(|e| RepositoryError::Database(e.into()))
}

/// Adapt a [`RepositoryError`] into the [`FromRow`](sqlx::FromRow) error channel.
fn decode_error(error: RepositoryError) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(error))
}

/// Parse a stored filing-status code, failing with [`RepositoryError::InvalidData`].
pub(crate) fn parse_filing_status_code(code: &str) -> Result<FilingStatusCode, RepositoryError> {
    FilingStatusCode::parse(code)
        .ok_or_else(|| RepositoryError::InvalidData(format!("Invalid status code: {code}")))
}
