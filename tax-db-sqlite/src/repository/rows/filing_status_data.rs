use rust_decimal::Decimal;
use sqlx::{FromRow, Row, sqlite::SqliteRow};

use super::decode_error;
use crate::decimal::{get_decimal, get_optional_decimal};

/// One denormalized row of the `get_filing_status_data` join: a filing status,
/// its standard deduction, and — via the `LEFT JOIN` — at most one tax bracket.
/// Bracket columns are `NULL` (so `min_income` is `None`) when the filing
/// status has no brackets for the requested year.
///
/// Grouping the rows back into `(FilingStatus, StandardDeduction, Vec<TaxBracket>)`
/// is left to the repository method, so there is no single-model `TryFrom`.
pub(crate) struct FilingStatusDataRow {
    pub(crate) status_id: i32,
    pub(crate) status_code: String,
    pub(crate) status_name: String,
    pub(crate) deduction_amount: Decimal,
    pub(crate) min_income: Option<Decimal>,
    pub(crate) max_income: Option<Decimal>,
    pub(crate) tax_rate: Decimal,
    pub(crate) base_tax: Decimal,
}

impl<'r> FromRow<'r, SqliteRow> for FilingStatusDataRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            status_id: row.try_get("status_id")?,
            status_code: row.try_get("status_code")?,
            status_name: row.try_get("status_name")?,
            deduction_amount: get_decimal(row, "deduction_amount").map_err(decode_error)?,
            min_income: get_optional_decimal(row, "min_income").map_err(decode_error)?,
            max_income: get_optional_decimal(row, "max_income").map_err(decode_error)?,
            tax_rate: get_decimal(row, "tax_rate").map_err(decode_error)?,
            base_tax: get_decimal(row, "base_tax").map_err(decode_error)?,
        })
    }
}
