//! Tax calculation modules for IRS Form 1040-ES.
//!
//! This module provides calculation logic for estimated tax computations,
//! organized by the various worksheets that comprise Form 1040-ES.

pub mod common;
pub mod worksheets;

pub use worksheets::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetError, EstimatedTaxWorksheetInput,
    EstimatedTaxWorksheetResult, SeWorksheet, SeWorksheetConfig, SeWorksheetError,
    SeWorksheetResult,
};
