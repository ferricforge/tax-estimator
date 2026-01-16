//! Main menu view for the tax estimator.

use cursive::align::HAlign;
use cursive::views::{Dialog, SelectView, TextView, LinearLayout};
use cursive::Cursive;

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

    let layout = LinearLayout::vertical()
        .child(TextView::new("Federal Estimated Tax Calculator").h_align(HAlign::Center))
        .child(TextView::new(""))  // Spacer
        .child(menu);

    let dialog = Dialog::around(layout)
        .title("Tax Estimator - Form 1040-ES")
        .padding_lrtb(2, 2, 1, 1);

    siv.add_layer(dialog);
}

/// Handles the user's menu selection.
fn handle_menu_selection(siv: &mut Cursive, action: &MenuAction) {
    match action {
        MenuAction::NewEstimate => show_new_estimate_placeholder(siv),
        MenuAction::LoadEstimate => show_load_estimate_placeholder(siv),
        MenuAction::Exit => siv.quit(),
    }
}

/// Placeholder for the new estimate workflow.
/// Future: This will navigate to SE Worksheet (if needed) then 1040-ES Worksheet.
fn show_new_estimate_placeholder(siv: &mut Cursive) {
    siv.add_layer(
        Dialog::text(
            "New Estimate workflow:\n\n\
             1. Self-Employment Tax (optional)\n\
             2. Estimated Tax (1040-ES)\n\n\
             Coming soon!"
        )
        .title("New Estimate")
        .button("OK", |s| { s.pop_layer(); })
        .h_align(HAlign::Center),
    );
}

/// Placeholder for loading saved estimates.
/// Future: This will list saved estimates from the database.
fn show_load_estimate_placeholder(siv: &mut Cursive) {
    siv.add_layer(
        Dialog::text("Load saved estimates from database.\n\nComing soon!")
            .title("Load Estimate")
            .button("OK", |s| { s.pop_layer(); })
            .h_align(HAlign::Center),
    );
}
