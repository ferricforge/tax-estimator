//! Estimate workflow coordinator.
//!
//! This view manages the multi-step process of creating or editing
//! a tax estimate, guiding the user through the SE worksheet and
//! the main 1040-ES estimated tax worksheet.

use cursive::align::HAlign;
use cursive::view::Resizable;
use cursive::views::{Dialog, DummyView, LinearLayout, SelectView, TextView};
use cursive::Cursive;

use super::est_tax_worksheet::show_est_tax_worksheet;
use super::se_worksheet::show_se_worksheet;
use super::status_bar::{build_status_bar, hints, KeyHint};
use crate::state::AppState;

/// Workflow step actions.
#[derive(Debug, Clone, Copy)]
enum WorkflowAction {
    SeWorksheet,
    EstTaxWorksheet,
    Back,
}

/// Display the estimate workflow screen.
///
/// This screen shows the two-step process and the completion status
/// of each step, allowing the user to navigate between them.
pub fn show_estimate_workflow(siv: &mut Cursive) {
    // Get current state to show completion status
    let (tax_year, se_done, est_done) = siv
        .with_user_data(|state: &mut AppState| {
            (
                state.tax_year,
                state.has_se_data(),
                state.has_est_tax_data(),
            )
        })
        .unwrap_or((2025, false, false));

    let header = LinearLayout::vertical()
        .child(
            TextView::new(format!("Tax Year {} - New Estimate", tax_year))
                .h_align(HAlign::Center)
                .full_width(),
        )
        .child(DummyView.fixed_height(1))
        .child(TextView::new(
            "Complete the worksheets below to calculate your\n\
             estimated tax. SE worksheet is optional if you\n\
             don't have self-employment income.",
        ));

    // Build menu with status indicators
    let se_status = if se_done { " ✓" } else { "" };
    let est_status = if est_done { " ✓" } else { "" };

    let menu = SelectView::new()
        .item(
            format!("1. Self-Employment Tax Worksheet{}", se_status),
            WorkflowAction::SeWorksheet,
        )
        .item(
            format!("2. Estimated Tax Worksheet (1040-ES){}", est_status),
            WorkflowAction::EstTaxWorksheet,
        )
        .item("← Back to Main Menu".to_string(), WorkflowAction::Back)
        .on_submit(handle_workflow_selection);

    let status = build_status_bar(&[
        KeyHint::new("↑↓", "Navigate"),
        hints::ENTER,
        hints::ESC,
        hints::CTRL_Q,
    ]);

    let layout = LinearLayout::vertical()
        .child(header)
        .child(DummyView.fixed_height(1))
        .child(menu)
        .child(DummyView.fixed_height(1))
        .child(status);

    let dialog = Dialog::around(layout)
        .title("Estimate Workflow")
        .padding_lrtb(1, 1, 1, 1);

    siv.add_layer(dialog);
}

/// Handle workflow menu selection.
fn handle_workflow_selection(siv: &mut Cursive, action: &WorkflowAction) {
    match action {
        WorkflowAction::SeWorksheet => {
            siv.pop_layer(); // Remove workflow screen
            show_se_worksheet(siv);
        }
        WorkflowAction::EstTaxWorksheet => {
            siv.pop_layer(); // Remove workflow screen
            show_est_tax_worksheet(siv);
        }
        WorkflowAction::Back => {
            siv.pop_layer();
        }
    }
}
