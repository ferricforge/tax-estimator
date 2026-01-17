//! Estimated Tax (1040-ES) worksheet view.
//!
//! This will implement the main Estimated Tax Worksheet from Form 1040-ES.
//! Currently a placeholder showing SE data carried forward.

use cursive::views::{Dialog, LinearLayout, TextView};
use cursive::Cursive;

use super::estimate_workflow::show_estimate_workflow;
use super::status_bar::{build_status_bar, hints};
use crate::state::AppState;

/// Display the Estimated Tax worksheet (placeholder).
pub fn show_est_tax_worksheet(siv: &mut Cursive) {
    // Get SE data if available
    let se_info = siv
        .with_user_data(|state: &mut AppState| {
            state.se_result.as_ref().map(|r| {
                format!(
                    "SE Tax (from SE Worksheet):     ${:.2}\n\
                     SE Deduction (for Line 11):    ${:.2}",
                    r.self_employment_tax, r.se_tax_deduction
                )
            })
        })
        .flatten()
        .unwrap_or_else(|| "No SE data - SE Worksheet not completed.".to_string());

    let content = format!(
        "Estimated Tax Worksheet (1040-ES)\n\
         ─────────────────────────────────\n\n\
         {}\n\n\
         This worksheet will include:\n\
         • Filing status selection\n\
         • Expected AGI input\n\
         • Deductions (standard/itemized)\n\
         • Tax calculation\n\
         • Credits and other taxes\n\
         • Required estimated payment\n\n\
         Coming in next iteration!",
        se_info
    );

    let status = build_status_bar(&[hints::ESC, hints::CTRL_Q]);

    let layout = LinearLayout::vertical()
        .child(TextView::new(content))
        .child(TextView::new(""))
        .child(status);

    let dialog = Dialog::around(layout)
        .title("1040-ES Worksheet")
        .button("Back", on_back)
        .padding_lrtb(1, 1, 0, 0);

    siv.add_layer(dialog);
}

/// Handle back button - return to workflow.
fn on_back(siv: &mut Cursive) {
    siv.pop_layer();
    show_estimate_workflow(siv);
}
