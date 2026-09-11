use rust_decimal::Decimal;
use sqlx::{FromRow, Row, sqlite::SqliteRow};
use tax_core::{RepositoryError, TaxBracket};

use super::decode_error;
use crate::decimal::{get_decimal, get_optional_decimal};

pub(crate) struct TaxBracketRow {
    tax_year: i32,
    filing_status_id: i32,
    min_income: Decimal,
    max_income: Option<Decimal>,
    tax_rate: Decimal,
    base_tax: Decimal,
}

impl<'r> FromRow<'r, SqliteRow> for TaxBracketRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            tax_year: row.try_get("tax_year")?,
            filing_status_id: row.try_get("filing_status_id")?,
            min_income: get_decimal(row, "min_income").map_err(decode_error)?,
            max_income: get_optional_decimal(row, "max_income").map_err(decode_error)?,
            tax_rate: get_decimal(row, "tax_rate").map_err(decode_error)?,
            base_tax: get_decimal(row, "base_tax").map_err(decode_error)?,
        })
    }
}

impl TryFrom<TaxBracketRow> for TaxBracket {
    type Error = RepositoryError;

    fn try_from(row: TaxBracketRow) -> Result<Self, Self::Error> {
        Ok(TaxBracket {
            tax_year: row.tax_year,
            filing_status_id: row.filing_status_id,
            min_income: row.min_income,
            max_income: row.max_income,
            tax_rate: row.tax_rate,
            base_tax: row.base_tax,
        })
    }
}
