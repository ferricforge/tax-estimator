//! UI views for the tax estimator application.
//!
//! This module organizes all view/screen implementations:
//! - `main_menu` - Application entry point and navigation
//! - `estimate_workflow` - Coordinates the multi-step estimate process
//! - `se_worksheet` - Self-Employment Tax worksheet form
//! - `est_tax_worksheet` - Estimated Tax (1040-ES) worksheet form (placeholder)
//!
//! Future modules:
//! - Results display
//! - Load/manage saved estimates

mod est_tax_worksheet;
mod estimate_workflow;
mod main_menu;
mod se_worksheet;
mod status_bar;

pub use main_menu::show_main_menu;
