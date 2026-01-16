//! UI views for the tax estimator application.
//!
//! This module organizes all view/screen implementations:
//! - `main_menu` - Application entry point and navigation
//!
//! Future modules:
//! - Self-Employment Tax worksheet form
//! - Estimated Tax (1040-ES) worksheet form
//! - Results display
//! - Load/manage saved estimates

mod main_menu;

pub use main_menu::show_main_menu;
