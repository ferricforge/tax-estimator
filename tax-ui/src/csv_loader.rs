//! CSV loader for tax estimate input data.
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
//! | `expected_deduction` | yes | decimal | |
//! | `expected_qbi_deduction`| no | decimal | Leave cell empty for `None` |
//! | `expected_amt` | no | decimal | Leave cell empty for `None` |
//! | `expected_credits` | no | decimal | Leave cell empty for `None` |
//! | `expected_other_taxes` | no | decimal | Leave cell empty for `None` |
//! | `expected_withholding` | no | decimal | Leave cell empty for `None` |
//! | `prior_year_tax` | no | decimal | Leave cell empty for `None` |
//! | `se_income` | no | decimal | Leave cell empty for `None` |
//! | `expected_crp_payments` | no | decimal | Leave cell empty for `None` |
//! | `expected_wages` | no | decimal | Leave cell empty for `None` |
//!
//! ### Filing Status Codes
//!
//! | Code | Meaning | ID (seed) |
//! |-------|-----------------------------|-----------|
//! | `S` | Single | 1 |
//! | `MFJ` | Married Filing Jointly | 2 |
//! | `MFS` | Married Filing Separately | 3 |
//! | `HOH` | Head of Household | 4 |
//! | `QSS` | Qualifying Surviving Spouse | 5 |
//!
//! ### Minimal example
//!
//! ```csv
//! tax_year,filing_status,expected_agi,expected_deduction
//! 2025,MFJ,150000.00,30000.00
//! ```
//!
//! ### Full example
//!
//! ```csv
//! tax_year,filing_status,expected_agi,expected_deduction,expected_qbi_deduction,expected_amt,expected_credits,expected_other_taxes,expected_withholding,prior_year_tax,se_income,expected_crp_payments,expected_wages
//! 2025,S,75000.00,14600.00,,,,,10000.00,12000.00,25000.00,,60000.00
//! 2025,MFJ,200000.00,29200.00,5000.00,,,500.00,35000.00,38000.00,,,180000.00
//! ```
use rust_decimal::Decimal;
use serde::Deserialize;
use tax_core::models::{FilingStatusCode, NewTaxEstimate};

// ---------------------------------------------------------------------------
// Serde-compatible row that mirrors the CSV layout exactly
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CsvRow {
    tax_year: i32,
    filing_status: String,
    expected_agi: Decimal,
    expected_deduction: Decimal,
    expected_qbi_deduction: Option<Decimal>,
    expected_amt: Option<Decimal>,
    expected_credits: Option<Decimal>,
    expected_other_taxes: Option<Decimal>,
    expected_withholding: Option<Decimal>,
    prior_year_tax: Option<Decimal>,
    se_income: Option<Decimal>,
    expected_crp_payments: Option<Decimal>,
    expected_wages: Option<Decimal>,
}

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

/// Errors that can occur while loading or converting CSV data.
#[derive(Debug, thiserror::Error)]
pub enum CsvLoadError {
    /// The underlying CSV deserialisation failed (bad structure, missing
    /// required column, type mismatch, etc.).
    #[error("CSV parse error: {0}")]
    Parse(#[from] csv::Error),

    /// A `filing_status` cell contained a value that is not one of the
    /// recognised codes.  The inner `String` is the offending value and
    /// `usize` is the 1-based row number (header = row 0).
    #[error("unrecognised filing status '{status}' on row {row}")]
    InvalidFilingStatus { status: String, row: usize },
}

// ---------------------------------------------------------------------------
// Core loader
// ---------------------------------------------------------------------------

/// Convert a single CSV row into a NewTaxEstimate.
///
/// row_number is 1-based (for error messages).
fn convert_row(
    row: CsvRow,
    row_number: usize,
) -> Result<NewTaxEstimate, CsvLoadError> {
    let code = FilingStatusCode::parse(&row.filing_status).ok_or_else(|| {
        CsvLoadError::InvalidFilingStatus {
            status: row.filing_status,
            row: row_number,
        }
    })?;

    Ok(NewTaxEstimate {
        tax_year: row.tax_year,
        filing_status_id: FilingStatusCode::filing_status_to_id(code),
        expected_agi: row.expected_agi,
        expected_deduction: row.expected_deduction,
        expected_qbi_deduction: row.expected_qbi_deduction,
        expected_amt: row.expected_amt,
        expected_credits: row.expected_credits,
        expected_other_taxes: row.expected_other_taxes,
        expected_withholding: row.expected_withholding,
        prior_year_tax: row.prior_year_tax,
        se_income: row.se_income,
        expected_crp_payments: row.expected_crp_payments,
        expected_wages: row.expected_wages,
    })
}

/// Parse CSV text (the full file contents as a &str) and return a vector of
/// NewTaxEstimate.  Rows are returned in file order.
///
/// # Errors
///
/// * [CsvLoadError::Parse] – if the CSV is structurally invalid or a
///   required field cannot be deserialised.
/// * [CsvLoadError::InvalidFilingStatus] – if any row contains an
///   unrecognised filing-status code.
pub fn load_from_str(input: &str) -> Result<Vec<NewTaxEstimate>, CsvLoadError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All) // tolerate whitespace around values
        .flexible(false) // strict column count
        .from_reader(input.as_bytes());

    reader
        .deserialize::<CsvRow>()
        .enumerate()
        .map(|(idx, result)| {
            let row = result?;
            let row_number = idx + 1; // 1-based for user-facing messages
            convert_row(row, row_number)
        })
        .collect()
}

/// Convenience wrapper: read a file from disk and delegate to [load_from_str].
///
/// # Errors
///
/// Returns an io::Error when the file cannot be read, or a
/// [CsvLoadError] when the contents are invalid.
pub fn load_from_file(
    path: &std::path::Path
) -> Result<Vec<NewTaxEstimate>, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let estimates = load_from_str(&contents)?;
    Ok(estimates)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    // -----------------------------------------------------------------------
    // Helper: the minimal set of columns
    // -----------------------------------------------------------------------
    const MINIMAL_CSV: &str = "\
tax_year,filing_status,expected_agi,expected_deduction
2025,S,75000.00,14600.00
";

    // -----------------------------------------------------------------------
    // Helper: every column populated
    // -----------------------------------------------------------------------
    const FULL_CSV: &str = "\
tax_year,filing_status,expected_agi,expected_deduction,expected_qbi_deduction,expected_amt,expected_credits,expected_other_taxes,expected_withholding,prior_year_tax,se_income,expected_crp_payments,expected_wages
2025,MFJ,200000.00,29200.00,5000.00,1500.00,500.00,300.00,35000.00,38000.00,40000.00,2000.00,180000.00
";

    // -----------------------------------------------------------------------
    // Helper: multiple rows with different statuses
    // -----------------------------------------------------------------------
    const MULTI_ROW_CSV: &str = "\
tax_year,filing_status,expected_agi,expected_deduction,se_income
2025,S,75000.00,14600.00,25000.00
2025,MFJ,200000.00,29200.00,
2025,MFS,90000.00,14600.00,10000.00
2025,HOH,60000.00,21900.00,
2025,QSS,110000.00,29200.00,15000.00
";

    // -----------------------------------------------------------------------
    // 1. Minimal CSV – only required columns, all optionals are None
    // -----------------------------------------------------------------------
    #[test]
    fn test_minimal_csv_parses_required_fields() {
        let estimates = load_from_str(MINIMAL_CSV).expect("should parse minimal CSV");

        assert_eq!(estimates.len(), 1);

        let e = &estimates[0];
        assert_eq!(e.tax_year, 2025);
        assert_eq!(e.filing_status_id, 1); // Single
        assert_eq!(e.expected_agi, dec!(75000.00));
        assert_eq!(e.expected_deduction, dec!(14600.00));
    }

    #[test]
    fn test_minimal_csv_optional_fields_are_none() {
        let estimates = load_from_str(MINIMAL_CSV).expect("should parse");
        let e = &estimates[0];

        assert!(e.expected_qbi_deduction.is_none());
        assert!(e.expected_amt.is_none());
        assert!(e.expected_credits.is_none());
        assert!(e.expected_other_taxes.is_none());
        assert!(e.expected_withholding.is_none());
        assert!(e.prior_year_tax.is_none());
        assert!(e.se_income.is_none());
        assert!(e.expected_crp_payments.is_none());
        assert!(e.expected_wages.is_none());
    }

    // -----------------------------------------------------------------------
    // 2. Full CSV – every column populated, verify exact values
    // -----------------------------------------------------------------------
    #[test]
    fn test_full_csv_all_fields_populated() {
        let estimates = load_from_str(FULL_CSV).expect("should parse full CSV");

        assert_eq!(estimates.len(), 1);

        let e = &estimates[0];
        assert_eq!(e.tax_year, 2025);
        assert_eq!(e.filing_status_id, 2); // MFJ
        assert_eq!(e.expected_agi, dec!(200000.00));
        assert_eq!(e.expected_deduction, dec!(29200.00));
        assert_eq!(e.expected_qbi_deduction, Some(dec!(5000.00)));
        assert_eq!(e.expected_amt, Some(dec!(1500.00)));
        assert_eq!(e.expected_credits, Some(dec!(500.00)));
        assert_eq!(e.expected_other_taxes, Some(dec!(300.00)));
        assert_eq!(e.expected_withholding, Some(dec!(35000.00)));
        assert_eq!(e.prior_year_tax, Some(dec!(38000.00)));
        assert_eq!(e.se_income, Some(dec!(40000.00)));
        assert_eq!(e.expected_crp_payments, Some(dec!(2000.00)));
        assert_eq!(e.expected_wages, Some(dec!(180000.00)));
    }

    // -----------------------------------------------------------------------
    // 3. Multiple rows – count, order, and per-status ID mapping
    // -----------------------------------------------------------------------
    #[test]
    fn test_multi_row_count_and_order() {
        let estimates = load_from_str(MULTI_ROW_CSV).expect("should parse multi-row CSV");
        assert_eq!(estimates.len(), 5);
    }

    #[test]
    fn test_multi_row_filing_status_ids() {
        let estimates = load_from_str(MULTI_ROW_CSV).expect("should parse");

        // Order matches file order
        assert_eq!(estimates[0].filing_status_id, 1); // S
        assert_eq!(estimates[1].filing_status_id, 2); // MFJ
        assert_eq!(estimates[2].filing_status_id, 3); // MFS
        assert_eq!(estimates[3].filing_status_id, 4); // HOH
        assert_eq!(estimates[4].filing_status_id, 5); // QSS
    }

    #[test]
    fn test_multi_row_optional_present_and_absent() {
        let estimates = load_from_str(MULTI_ROW_CSV).expect("should parse");

        // Row 0 (S): se_income is present
        assert_eq!(estimates[0].se_income, Some(dec!(25000.00)));

        // Row 1 (MFJ): se_income is empty → None
        assert!(estimates[1].se_income.is_none());

        // Row 2 (MFS): se_income is present
        assert_eq!(estimates[2].se_income, Some(dec!(10000.00)));
    }

    // -----------------------------------------------------------------------
    // 4. Parameterized filing-status tests
    // -----------------------------------------------------------------------
    #[test]
    fn test_filing_status_codes_map_to_correct_ids() {
        let test_cases = [("S", 1), ("MFJ", 2), ("MFS", 3), ("HOH", 4), ("QSS", 5)];

        for (code, expected_id) in test_cases {
            let csv = format!(
                "tax_year,filing_status,expected_agi,expected_deduction\n2025,{code},1.00,1.00\n"
            );
            let estimates = load_from_str(&csv)
                .unwrap_or_else(|e| panic!("failed to parse CSV for code '{code}': {e}"));

            assert_eq!(
                estimates[0].filing_status_id, expected_id,
                "filing status '{code}' should map to id {expected_id}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 5. Error: unrecognised filing status code
    // -----------------------------------------------------------------------
    #[test]
    fn test_invalid_filing_status_returns_error() {
        let csv = "tax_year,filing_status,expected_agi,expected_deduction\n2025,BOGUS,1.00,1.00\n";
        let result = load_from_str(csv);

        assert!(result.is_err());

        match result.unwrap_err() {
            CsvLoadError::InvalidFilingStatus { status, row } => {
                assert_eq!(status, "BOGUS");
                assert_eq!(row, 1); // first data row
            }
            other => panic!("expected InvalidFilingStatus, got {:?}", other),
        }
    }

    #[test]
    fn test_invalid_filing_status_on_second_row_reports_correct_row() {
        let csv = "\
tax_year,filing_status,expected_agi,expected_deduction
2025,S,1.00,1.00
2025,NOPE,2.00,2.00
";
        let result = load_from_str(csv);
        assert!(result.is_err());

        match result.unwrap_err() {
            CsvLoadError::InvalidFilingStatus { status, row } => {
                assert_eq!(status, "NOPE");
                assert_eq!(row, 2); // second data row
            }
            other => panic!("expected InvalidFilingStatus, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // 6. Error: missing required column
    // -----------------------------------------------------------------------
    #[test]
    fn test_missing_required_column_returns_parse_error() {
        // `expected_agi` is missing entirely from the header
        let csv = "tax_year,filing_status,expected_deduction\n2025,S,14600.00\n";
        let result = load_from_str(csv);
        assert!(result.is_err());

        match result.unwrap_err() {
            CsvLoadError::Parse(_) => { /* expected */ }
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // 7. Error: non-numeric value in a Decimal field
    // -----------------------------------------------------------------------
    #[test]
    fn test_non_numeric_decimal_returns_parse_error() {
        let csv = "tax_year,filing_status,expected_agi,expected_deduction\n2025,S,not_a_number,14600.00\n";
        let result = load_from_str(csv);
        assert!(result.is_err());

        match result.unwrap_err() {
            CsvLoadError::Parse(_) => { /* expected */ }
            other => panic!("expected Parse error, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // 8. Error: completely empty input (no header)
    // -----------------------------------------------------------------------
    #[test]
    fn test_empty_input_returns_empty_vec() {
        // A header with no data rows is valid — zero estimates.
        let csv = "tax_year,filing_status,expected_agi,expected_deduction\n";
        let estimates = load_from_str(csv).expect("header-only CSV is valid");
        assert!(estimates.is_empty());
    }

    #[test]
    fn test_completely_empty_string_returns_empty_vec() {
        // Truly empty — csv crate sees no header and produces zero records.
        // With `has_headers(true)` and no content, deserialize yields nothing.
        let estimates = load_from_str("").expect("empty string yields zero rows");
        assert!(estimates.is_empty());
    }

    // -----------------------------------------------------------------------
    // 9. Whitespace tolerance: spaces around values are trimmed
    // -----------------------------------------------------------------------
    #[test]
    fn test_whitespace_around_values_is_trimmed() {
        let csv = "\
tax_year , filing_status , expected_agi , expected_deduction
2025 , MFJ , 100000.00 , 29200.00
";
        let estimates = load_from_str(csv).expect("should tolerate surrounding whitespace");
        assert_eq!(estimates.len(), 1);
        assert_eq!(estimates[0].filing_status_id, 2);
        assert_eq!(estimates[0].expected_agi, dec!(100000.00));
    }

    // -----------------------------------------------------------------------
    // 10. Column order does not matter
    // -----------------------------------------------------------------------
    #[test]
    fn test_column_order_does_not_matter() {
        // Columns deliberately shuffled relative to the canonical order
        let csv = "\
expected_deduction,filing_status,expected_agi,tax_year,se_income
14600.00,S,75000.00,2025,25000.00
";
        let estimates = load_from_str(csv).expect("column order should not matter");
        assert_eq!(estimates.len(), 1);
        assert_eq!(estimates[0].tax_year, 2025);
        assert_eq!(estimates[0].filing_status_id, 1);
        assert_eq!(estimates[0].expected_agi, dec!(75000.00));
        assert_eq!(estimates[0].expected_deduction, dec!(14600.00));
        assert_eq!(estimates[0].se_income, Some(dec!(25000.00)));
    }

    // -----------------------------------------------------------------------
    // 11. Decimal precision is preserved
    // -----------------------------------------------------------------------
    #[test]
    fn test_decimal_precision_preserved() {
        let csv =
            "tax_year,filing_status,expected_agi,expected_deduction\n2025,S,12345.67,890.12\n";
        let estimates = load_from_str(csv).expect("should parse");

        // Compare with exact Decimal values constructed from strings to avoid
        // any floating-point nonsense.
        let agi: Decimal = "12345.67".parse().unwrap();
        let ded: Decimal = "890.12".parse().unwrap();

        assert_eq!(estimates[0].expected_agi, agi);
        assert_eq!(estimates[0].expected_deduction, ded);
    }

    // -----------------------------------------------------------------------
    // 12. Integer-only decimals (no fractional part) parse correctly
    // -----------------------------------------------------------------------
    #[test]
    fn test_integer_decimals_without_fractional_part() {
        let csv = "tax_year,filing_status,expected_agi,expected_deduction\n2025,S,75000,14600\n";
        let estimates = load_from_str(csv).expect("should parse integers as Decimal");

        assert_eq!(estimates[0].expected_agi, dec!(75000));
        assert_eq!(estimates[0].expected_deduction, dec!(14600));
    }
}
