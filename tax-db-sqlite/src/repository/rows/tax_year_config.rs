use rust_decimal::Decimal;
use sqlx::{FromRow, Row, sqlite::SqliteRow};
use tax_core::{RepositoryError, TaxYearConfig};

use super::decode_error;
use crate::decimal::get_decimal;

pub(crate) struct TaxYearConfigRow {
    tax_year: i32,
    ss_wage_max: Decimal,
    ss_tax_rate: Decimal,
    medicare_tax_rate: Decimal,
    se_tax_deduct_pcnt: Decimal,
    se_deduction_factor: Decimal,
    req_pmnt_threshold: Decimal,
    min_se_threshold: Decimal,
}

impl<'r> FromRow<'r, SqliteRow> for TaxYearConfigRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            tax_year: row.try_get("tax_year")?,
            ss_wage_max: get_decimal(row, "ss_wage_max").map_err(decode_error)?,
            ss_tax_rate: get_decimal(row, "ss_tax_rate").map_err(decode_error)?,
            medicare_tax_rate: get_decimal(row, "medicare_tax_rate").map_err(decode_error)?,
            se_tax_deduct_pcnt: get_decimal(row, "se_tax_deductible_percentage")
                .map_err(decode_error)?,
            se_deduction_factor: get_decimal(row, "se_deduction_factor").map_err(decode_error)?,
            req_pmnt_threshold: get_decimal(row, "required_payment_threshold")
                .map_err(decode_error)?,
            min_se_threshold: get_decimal(row, "min_se_threshold").map_err(decode_error)?,
        })
    }
}

impl TryFrom<TaxYearConfigRow> for TaxYearConfig {
    type Error = RepositoryError;

    fn try_from(row: TaxYearConfigRow) -> Result<Self, Self::Error> {
        Ok(TaxYearConfig {
            tax_year: row.tax_year,
            ss_wage_max: row.ss_wage_max,
            ss_tax_rate: row.ss_tax_rate,
            medicare_tax_rate: row.medicare_tax_rate,
            se_tax_deduct_pcnt: row.se_tax_deduct_pcnt,
            se_deduction_factor: row.se_deduction_factor,
            req_pmnt_threshold: row.req_pmnt_threshold,
            min_se_threshold: row.min_se_threshold,
        })
    }
}
