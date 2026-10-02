//! Tax calculation modules for IRS Form 1040-ES and Form 8995.
//!
//! This module provides calculation logic for estimated tax computations,
//! organized by the various worksheets that comprise Form 1040-ES, and for
//! the simplified qualified business income deduction on Form 8995.

pub mod common;
pub mod worksheets;

pub use worksheets::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetContext, EstimatedTaxWorksheetError,
    EstimatedTaxWorksheetInput, EstimatedTaxWorksheetResult, QBI_DEDUCTION_RATE, QbiWorksheet,
    QbiWorksheetConfig, QbiWorksheetError, QbiWorksheetInput, QbiWorksheetResult, SeWorksheet,
    SeWorksheetConfig, SeWorksheetError, SeWorksheetResult, qbi_taxable_income_threshold,
    taxable_income_before_qbi,
};
