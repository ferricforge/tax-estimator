//! Integration tests that exercise the loader against an on-disk fixture file.
//!
//! These complement the unit tests inside csv_loader.rs (which all use
//! inline string literals) by verifying that the full read-from-disk path
//! works end-to-end.

use std::path::Path;

use rust_decimal_macros::dec;
use tax_ui::csv_loader;

/// Path to the sample CSV shipped with the test fixtures.
fn fixture_path() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample_estimates.csv")
        .leak() // fine — this is test-only, runs once
}

#[test]
fn test_load_fixture_file_succeeds() {
    let estimates =
        csv_loader::load_from_file(fixture_path()).expect("fixture file should load without error");

    // The fixture has exactly 3 rows.
    assert_eq!(estimates.len(), 3);
}

#[test]
fn test_load_fixture_first_row_single() {
    let estimates = csv_loader::load_from_file(fixture_path()).unwrap();
    let e = &estimates[0];

    assert_eq!(e.tax_year, 2025);
    assert_eq!(e.filing_status_id, 1); // Single
    assert_eq!(e.expected_agi, dec!(75000.00));
    assert_eq!(e.expected_deduction, dec!(14600.00));

    // Optionals that are empty in the fixture
    assert!(e.expected_qbi_deduction.is_none());
    assert!(e.expected_amt.is_none());
    assert!(e.expected_credits.is_none());
    assert!(e.expected_other_taxes.is_none());

    // Optionals that are populated
    assert_eq!(e.expected_withholding, Some(dec!(10000.00)));
    assert_eq!(e.prior_year_tax, Some(dec!(12000.00)));
    assert_eq!(e.se_income, Some(dec!(25000.00)));
    assert!(e.expected_crp_payments.is_none());
    assert_eq!(e.expected_wages, Some(dec!(60000.00)));
}

#[test]
fn test_load_fixture_second_row_mfj() {
    let estimates = csv_loader::load_from_file(fixture_path()).unwrap();
    let e = &estimates[1];

    assert_eq!(e.filing_status_id, 2); // MFJ
    assert_eq!(e.expected_qbi_deduction, Some(dec!(5000.00)));
    assert_eq!(e.expected_credits, Some(dec!(500.00)));
    assert_eq!(e.expected_wages, Some(dec!(180000.00)));
    assert!(e.se_income.is_none());
}

#[test]
fn test_load_fixture_third_row_hoh() {
    let estimates = csv_loader::load_from_file(fixture_path()).unwrap();
    let e = &estimates[2];

    assert_eq!(e.filing_status_id, 4); // HOH
    assert_eq!(e.expected_agi, dec!(58000.00));
    assert_eq!(e.expected_deduction, dec!(21900.00));
    assert_eq!(e.prior_year_tax, Some(dec!(11000.00)));
    assert_eq!(e.expected_wages, Some(dec!(55000.00)));
    assert!(e.se_income.is_none());
    assert!(e.expected_crp_payments.is_none());
}

#[test]
fn test_load_nonexistent_file_returns_err() {
    let bad_path = Path::new("/this/path/does/not/exist.csv");
    let result = csv_loader::load_from_file(bad_path);
    assert!(result.is_err());
}
