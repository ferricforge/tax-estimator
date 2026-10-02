//! IRS worksheet and form implementations.
//!
//! This module contains the calculation logic for the worksheets that
//! comprise Form 1040-ES estimated tax calculations, and for Form 8995, the
//! simplified qualified business income deduction computation.

pub mod est_tax;
pub mod qbi;
pub mod self_emp;

pub use est_tax::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetContext, EstimatedTaxWorksheetError,
    EstimatedTaxWorksheetInput, EstimatedTaxWorksheetResult,
};
pub use qbi::{
    QBI_DEDUCTION_RATE, QbiWorksheet, QbiWorksheetConfig, QbiWorksheetError, QbiWorksheetInput,
    QbiWorksheetResult, qbi_taxable_income_threshold, taxable_income_before_qbi,
};
pub use self_emp::{SeWorksheet, SeWorksheetConfig, SeWorksheetError, SeWorksheetResult};
