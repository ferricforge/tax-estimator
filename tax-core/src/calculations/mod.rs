//! Tax calculation modules for IRS Form 1040-ES worksheets.
//!
//! This module provides calculation logic for estimated tax computations,
//! organized by the various worksheets that comprise Form 1040-ES.

pub mod se_worksheet;

pub use se_worksheet::{SeWorksheet, SeWorksheetConfig, SeWorksheetError, SeWorksheetResult};
