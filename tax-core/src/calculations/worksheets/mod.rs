//! IRS Form 1040-ES worksheet implementations.
//!
//! This module contains the calculation logic for the various worksheets
//! that comprise Form 1040-ES estimated tax calculations.

pub mod est_tax;
pub mod self_emp;

pub use est_tax::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetError, EstimatedTaxWorksheetInput,
    EstimatedTaxWorksheetResult,
};
pub use self_emp::{SeWorksheet, SeWorksheetConfig, SeWorksheetError, SeWorksheetResult};
