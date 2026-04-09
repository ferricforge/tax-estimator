//! CSV loader for canonical tax estimate input data.
//!
//! ## CSV Format
//!
//! The expected CSV format uses the following columns. Column order does **not**
//! matter (headers are matched by name). All header names are case-sensitive
//! and must match exactly.
//!
//! | Column | Required | Type | Notes |
//! |-------------------------|----------|---------|--------------------------------------------|
//! | `tax_year` | yes | integer | e.g. `2025` |
//! | `filing_status` | yes | string | One of: `S`, `MFJ`, `MFS`, `HOH`, `QSS` |
//! | `expected_agi` | yes | decimal | e.g. `75000.00` |
//! | `expected_deduction` | yes | decimal | Deduction amount, regardless of source |
//! | `expected_qbi_deduction`| no | decimal | Leave cell empty for `None` |
//! | `expected_amt` | no | decimal | Leave cell empty for `None` |
//! | `expected_credits` | no | decimal | Leave cell empty for `None` |
//! | `expected_other_taxes` | no | decimal | Leave cell empty for `None` |
//! | `expected_withholding` | no | decimal | Leave cell empty for `None` |
//! | `prior_year_tax` | no | decimal | Leave cell empty for `None` |
//! | `se_income` | no | decimal | Leave cell empty for `None` |
//! | `expected_crp_payments` | no | decimal | Leave cell empty for `None` |
//! | `expected_wages` | no | decimal | Leave cell empty for `None` |

use rust_decimal::Decimal;
use serde::Deserialize;
use tax_core::{FilingStatusCode, TaxEstimateInput};

#[derive(Debug, Deserialize)]
struct CsvRow {
    tax_year: i32,
    filing_status: String,
    expected_agi: Decimal,
    expected_deduction: Decimal,
    se_income: Option<Decimal>,
    expected_crp_payments: Option<Decimal>,
    expected_wages: Option<Decimal>,
    expected_qbi_deduction: Option<Decimal>,
    expected_amt: Option<Decimal>,
    expected_credits: Option<Decimal>,
    expected_other_taxes: Option<Decimal>,
    expected_withholding: Option<Decimal>,
    prior_year_tax: Option<Decimal>,
}

/// Errors that can occur while loading or converting CSV data.
#[derive(Debug, thiserror::Error)]
pub enum CsvLoadError {
    /// The underlying CSV deserialisation failed (bad structure, missing
    /// required column, type mismatch, etc.).
    #[error("CSV parse error: {0}")]
    Parse(#[from] csv::Error),

    /// A `filing_status` cell contained a value that is not one of the
    /// recognised codes.
    #[error("unrecognised filing status '{status}' on row {row}")]
    InvalidFilingStatus { status: String, row: usize },
}

/// Convert a single CSV row into a [`TaxEstimateInput`].
fn convert_row(
    row: CsvRow,
    row_number: usize,
) -> Result<TaxEstimateInput, CsvLoadError> {
    let filing_status = FilingStatusCode::parse(&row.filing_status).ok_or_else(|| {
        CsvLoadError::InvalidFilingStatus {
            status: row.filing_status.clone(),
            row: row_number,
        }
    })?;

    Ok(TaxEstimateInput {
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
    })
}

/// Parse CSV text and return canonical estimate inputs in file order.
pub fn load_from_str(input: &str) -> Result<Vec<TaxEstimateInput>, CsvLoadError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .flexible(false)
        .from_reader(input.as_bytes());

    reader
        .deserialize::<CsvRow>()
        .enumerate()
        .map(|(idx, result)| {
            let row = result?;
            convert_row(row, idx + 1)
        })
        .collect()
}

/// Convenience wrapper: read a file from disk and delegate to [`load_from_str`].
pub fn load_from_file(
    path: &std::path::Path
) -> Result<Vec<TaxEstimateInput>, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let estimates = load_from_str(&contents)?;
    Ok(estimates)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    const MINIMAL_CSV: &str = "\
tax_year,filing_status,expected_agi,expected_deduction
2025,S,75000.00,14600.00
";

    const FULL_CSV: &str = "\
tax_year,filing_status,expected_agi,expected_deduction,expected_qbi_deduction,expected_amt,expected_credits,expected_other_taxes,expected_withholding,prior_year_tax,se_income,expected_crp_payments,expected_wages
2025,MFJ,200000.00,32000.00,5000.00,1500.00,500.00,300.00,35000.00,38000.00,40000.00,2000.00,180000.00
";

    #[test]
    fn minimal_csv_parses_deduction_amount() {
        let estimates = load_from_str(MINIMAL_CSV).expect("should parse minimal CSV");

        assert_eq!(estimates.len(), 1);
        assert_eq!(estimates[0].tax_year, 2025);
        assert_eq!(estimates[0].filing_status, FilingStatusCode::Single);
        assert_eq!(estimates[0].expected_agi, dec!(75000.00));
        assert_eq!(estimates[0].expected_deduction, dec!(14600.00));
    }

    #[test]
    fn full_csv_parses_optionals() {
        let estimates = load_from_str(FULL_CSV).expect("should parse full CSV");
        let estimate = &estimates[0];

        assert_eq!(
            estimate.filing_status,
            FilingStatusCode::MarriedFilingJointly
        );
        assert_eq!(estimate.expected_deduction, dec!(32000.00));
        assert_eq!(estimate.expected_qbi_deduction, Some(dec!(5000.00)));
        assert_eq!(estimate.expected_amt, Some(dec!(1500.00)));
        assert_eq!(estimate.expected_credits, Some(dec!(500.00)));
        assert_eq!(estimate.expected_other_taxes, Some(dec!(300.00)));
        assert_eq!(estimate.expected_withholding, Some(dec!(35000.00)));
        assert_eq!(estimate.prior_year_tax, Some(dec!(38000.00)));
        assert_eq!(estimate.se_income, Some(dec!(40000.00)));
        assert_eq!(estimate.expected_crp_payments, Some(dec!(2000.00)));
        assert_eq!(estimate.expected_wages, Some(dec!(180000.00)));
    }

    #[test]
    fn invalid_filing_status_returns_error() {
        let csv = "tax_year,filing_status,expected_agi,expected_deduction\n2025,BOGUS,1.00,1.00\n";
        let error = load_from_str(csv).expect_err("expected invalid filing status");

        assert_eq!(
            error.to_string(),
            "unrecognised filing status 'BOGUS' on row 1"
        );
    }
}
