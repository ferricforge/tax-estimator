use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{FromRow, Row, sqlite::SqliteRow};
use tax_core::{RepositoryError, TaxEstimate, TaxEstimateComputed, TaxEstimateInput};

use super::{decode_error, field, parse_filing_status_code};
use crate::decimal::{get_decimal, get_optional_decimal};

pub(crate) struct EstimateRow {
    id: i64,
    tax_year: i32,
    filing_status_code: String,
    se_income: Option<Decimal>,
    expected_crp_payments: Option<Decimal>,
    expected_wages: Option<Decimal>,
    expected_agi: Decimal,
    expected_deduction: Decimal,
    expected_qbi_deduction: Option<Decimal>,
    expected_amt: Option<Decimal>,
    expected_credits: Option<Decimal>,
    expected_other_taxes: Option<Decimal>,
    expected_withholding: Option<Decimal>,
    prior_year_tax: Option<Decimal>,
    calculated_se_tax: Option<Decimal>,
    calculated_total_tax: Option<Decimal>,
    calculated_required_payment: Option<Decimal>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl EstimateRow {
    /// Decode every `tax_estimate` column from `row`, taking the filing-status
    /// code from the caller instead of the row.
    ///
    /// The insert/upsert path uses this with the code already known from the
    /// input, so its `RETURNING` clause need not join to `filing_status`.
    pub(crate) fn from_row_with_code(
        row: &SqliteRow,
        filing_status_code: String,
    ) -> Result<Self, RepositoryError> {
        Ok(Self {
            id: field(row, "id")?,
            tax_year: field(row, "tax_year")?,
            filing_status_code,
            se_income: get_optional_decimal(row, "se_income")?,
            expected_crp_payments: get_optional_decimal(row, "expected_crp_payments")?,
            expected_wages: get_optional_decimal(row, "expected_wages")?,
            expected_agi: get_decimal(row, "expected_agi")?,
            expected_deduction: get_decimal(row, "expected_deduction")?,
            expected_qbi_deduction: get_optional_decimal(row, "expected_qbi_deduction")?,
            expected_amt: get_optional_decimal(row, "expected_amt")?,
            expected_credits: get_optional_decimal(row, "expected_credits")?,
            expected_other_taxes: get_optional_decimal(row, "expected_other_taxes")?,
            expected_withholding: get_optional_decimal(row, "expected_withholding")?,
            prior_year_tax: get_optional_decimal(row, "prior_year_tax")?,
            calculated_se_tax: get_optional_decimal(row, "calculated_se_tax")?,
            calculated_total_tax: get_optional_decimal(row, "calculated_total_tax")?,
            calculated_required_payment: get_optional_decimal(row, "calculated_required_payment")?,
            created_at: field(row, "created_at")?,
            updated_at: field(row, "updated_at")?,
        })
    }
}

impl<'r> FromRow<'r, SqliteRow> for EstimateRow {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let filing_status_code: String = row.try_get("filing_status_code")?;
        Self::from_row_with_code(row, filing_status_code).map_err(decode_error)
    }
}

impl TryFrom<EstimateRow> for TaxEstimate {
    type Error = RepositoryError;

    fn try_from(row: EstimateRow) -> Result<Self, Self::Error> {
        let filing_status = parse_filing_status_code(&row.filing_status_code)?;

        let computed = match (
            row.calculated_se_tax,
            row.calculated_total_tax,
            row.calculated_required_payment,
        ) {
            (None, None, None) => None,
            (Some(se_tax), Some(total_tax), Some(required_payment)) => Some(TaxEstimateComputed {
                se_tax,
                total_tax,
                required_payment,
            }),
            _ => {
                return Err(RepositoryError::InvalidData(
                    "tax_estimate row has partially populated calculated fields".to_string(),
                ));
            }
        };

        Ok(TaxEstimate {
            id: row.id,
            input: TaxEstimateInput {
                tax_year: row.tax_year,
                filing_status,
                se_income: row.se_income,
                expected_crp_payments: row.expected_crp_payments,
                expected_wages: row.expected_wages,
                expected_agi: row.expected_agi,
                expected_deduction: row.expected_deduction,
                expected_qbi_deduction: row.expected_qbi_deduction,
                expected_amt: row.expected_amt,
                expected_credits: row.expected_credits,
                expected_other_taxes: row.expected_other_taxes,
                expected_withholding: row.expected_withholding,
                prior_year_tax: row.prior_year_tax,
            },
            computed,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}
