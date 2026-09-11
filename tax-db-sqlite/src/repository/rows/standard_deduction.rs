use rust_decimal::Decimal;
use sqlx::{FromRow, Row, sqlite::SqliteRow};
use tax_core::{RepositoryError, StandardDeduction};

use super::decode_error;
use crate::decimal::get_decimal;

pub(crate) struct StandardDeductionRow {
    tax_year: i32,
    filing_status_id: i32,
    amount: Decimal,
}

impl<'r> FromRow<'r, SqliteRow> for StandardDeductionRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            tax_year: row.try_get("tax_year")?,
            filing_status_id: row.try_get("filing_status_id")?,
            amount: get_decimal(row, "amount").map_err(decode_error)?,
        })
    }
}

impl TryFrom<StandardDeductionRow> for StandardDeduction {
    type Error = RepositoryError;

    fn try_from(row: StandardDeductionRow) -> Result<Self, Self::Error> {
        Ok(StandardDeduction {
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            amount: row.amount,
        })
    }
}
