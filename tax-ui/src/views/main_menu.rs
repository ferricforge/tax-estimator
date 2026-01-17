//! Main menu view for the tax estimator.

use cursive::align::HAlign;
use cursive::view::Resizable;
use cursive::views::{Dialog, DummyView, LinearLayout, SelectView, TextView};
use cursive::Cursive;

use super::estimate_workflow::show_estimate_workflow;
use super::status_bar::{build_status_bar, hints, KeyHint};
use crate::state::AppState;

/// Menu actions available from the main menu.
#[derive(Debug, Clone, Copy)]
enum MenuAction {
    NewEstimate,
    LoadEstimate,
    Exit,
}

/// Displays the main menu as the root view.
pub fn show_main_menu(siv: &mut Cursive) {
    let menu = SelectView::new()
        .item("New Estimate", MenuAction::NewEstimate)
        .item("Load Estimate", MenuAction::LoadEstimate)
        .item("Exit", MenuAction::Exit)
        .on_submit(handle_menu_selection);

    // Get tax year from state for display
    let tax_year = siv
        .with_user_data(|state: &mut AppState| state.tax_year)
        .unwrap_or(2025);

    let header = TextView::new(format!("Tax Year {}", tax_year))
        .h_align(HAlign::Center)
        .full_width();

    let status = build_status_bar(&[
        KeyHint::new("↑↓", "Navigate"),
        hints::ENTER,
        hints::CTRL_Q,
    ]);

    let layout = LinearLayout::vertical()
        .child(header)
        .child(DummyView.fixed_height(1))
        .child(menu)
        .child(DummyView.fixed_height(1))
        .child(status);

    let dialog = Dialog::around(layout)
        .title("Tax Estimator - Form 1040-ES")
        .padding_lrtb(2, 2, 1, 1);

    siv.add_layer(dialog);
}

/// Handles the user's menu selection.
fn handle_menu_selection(siv: &mut Cursive, action: &MenuAction) {
    match action {
        MenuAction::NewEstimate => start_new_estimate(siv),
        MenuAction::LoadEstimate => show_load_estimate_placeholder(siv),
        MenuAction::Exit => siv.quit(),
    }
}

/// Start the new estimate workflow.
fn start_new_estimate(siv: &mut Cursive) {
    // Clear any existing estimate data
    siv.with_user_data(|state: &mut AppState| {
        state.clear_estimate();
    });

    // Show the workflow screen
    show_estimate_workflow(siv);
}

/// Placeholder for loading saved estimates.
fn show_load_estimate_placeholder(siv: &mut Cursive) {
    siv.add_layer(
        Dialog::text(
            "Load saved estimates from database.\n\n\
             Future: List saved estimates, select one,\n\
             then open the estimate workflow to review/edit.",
        )
        .title("Load Estimate")
        .button("OK", |s| {
            s.pop_layer();
        }),
    );
}
