//! Application state for the tax estimator UI.
//!
//! This module holds the in-memory state that persists across views,
//! allowing data to flow between worksheets before final database persistence.

use rust_decimal::Decimal;
use tax_core::calculations::SeWorksheetResult;

/// Application-wide state stored in Cursive's user data.
///
/// This holds intermediate calculation results as the user progresses
/// through the worksheets. Data here is not persisted until explicitly saved.
#[derive(Debug, Clone, Default)]
pub struct AppState {
    /// Current tax year for calculations (defaults to 2025)
    pub tax_year: i32,

    // SE Worksheet inputs
    /// Net profit from self-employment (Schedule C, F, etc.)
    pub se_income: Option<Decimal>,
    /// Conservation Reserve Program payments
    pub crp_payments: Option<Decimal>,
    /// Wages subject to social security tax
    pub wages: Option<Decimal>,

    /// Saved SE worksheet calculation result.
    /// Populated when user saves the SE worksheet.
    pub se_result: Option<SeWorksheetResult>,

    /// Flag indicating the estimated tax worksheet has been completed.
    pub est_tax_completed: bool,
}

impl AppState {
    /// Create a new application state for the given tax year.
    pub fn new(tax_year: i32) -> Self {
        Self {
            tax_year,
            ..Default::default()
        }
    }

    /// Check if SE worksheet has been completed.
    pub fn has_se_data(&self) -> bool {
        self.se_result.is_some()
    }

    /// Check if estimated tax worksheet has been completed.
    pub fn has_est_tax_data(&self) -> bool {
        self.est_tax_completed
    }

    /// Clear all estimate data for starting fresh.
    pub fn clear_estimate(&mut self) {
        self.se_income = None;
        self.crp_payments = None;
        self.wages = None;
        self.se_result = None;
        self.est_tax_completed = false;
    }
}
