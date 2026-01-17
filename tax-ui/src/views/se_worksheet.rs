//! Self-Employment Tax worksheet view.
//!
//! This implements the SE Tax and Deduction Worksheet from Form 1040-ES,
//! allowing users to input self-employment income and see calculated
//! SE tax in real-time.

use cursive::align::HAlign;
use cursive::event::Key;
use cursive::view::{Nameable, Resizable};
use cursive::views::{Dialog, DummyView, EditView, LinearLayout, Panel, TextView};
use cursive::Cursive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use tax_core::calculations::{SeWorksheet, SeWorksheetConfig, SeWorksheetResult};

use super::status_bar::{build_status_bar, hints};
use crate::state::AppState;

// View names for accessing components
const SE_INCOME_FIELD: &str = "se_income_input";
const CRP_PAYMENTS_FIELD: &str = "crp_payments_input";
const WAGES_FIELD: &str = "wages_input";
const RESULTS_VIEW: &str = "se_results_display";

/// Display the Self-Employment Tax worksheet form.
pub fn show_se_worksheet(siv: &mut Cursive) {
    // Load existing values from state if any
    let (se_income, crp, wages) = siv
        .with_user_data(|state: &mut AppState| {
            (state.se_income, state.crp_payments, state.wages)
        })
        .unwrap_or((None, None, None));

    let form = build_form(se_income, crp, wages);

    let status = build_status_bar(&[hints::TAB, hints::SHIFT_TAB, hints::ESC, hints::ENTER]);

    let layout = LinearLayout::vertical()
        .child(form)
        .child(DummyView.fixed_height(1))
        .child(status);

    let dialog = Dialog::around(layout)
        .title("Self-Employment Tax Worksheet")
        .button("Cancel", |s| {
            s.pop_layer();
        })
        .button("Save", save_worksheet);

    siv.add_layer(dialog);

    // Add Enter key to save from anywhere in the form
    siv.add_layer_cb(|s| {
        s.add_global_callback(Key::Enter, |s| {
            // Only trigger save if we're in the SE worksheet
            if s.screen().len() > 1 {
                save_worksheet(s);
            }
        });
    });

    // Trigger initial calculation
    recalculate(siv);
}

/// Build the complete form layout.
fn build_form(
    se_income: Option<Decimal>,
    crp: Option<Decimal>,
    wages: Option<Decimal>,
) -> LinearLayout {
    // Input section
    let inputs = build_input_section(se_income, crp, wages);

    // Results section
    let results = TextView::new(format_results(None))
        .with_name(RESULTS_VIEW)
        .full_width();

    LinearLayout::vertical()
        .child(Panel::new(inputs).title("Enter Your Information"))
        .child(DummyView.fixed_height(1))
        .child(Panel::new(results).title("Calculated Results (Live)"))
}

/// Build the input fields section.
fn build_input_section(
    se_income: Option<Decimal>,
    crp: Option<Decimal>,
    wages: Option<Decimal>,
) -> LinearLayout {
    let description = TextView::new(
        "Enter your self-employment income to calculate SE tax.\n\
         Leave fields empty or zero if not applicable.",
    )
    .h_align(HAlign::Left);

    let se_income_field = EditView::new()
        .content(format_input(se_income))
        .on_edit(|s, _, _| recalculate(s))
        .with_name(SE_INCOME_FIELD)
        .fixed_width(15);

    let crp_field = EditView::new()
        .content(format_input(crp))
        .on_edit(|s, _, _| recalculate(s))
        .with_name(CRP_PAYMENTS_FIELD)
        .fixed_width(15);

    let wages_field = EditView::new()
        .content(format_input(wages))
        .on_edit(|s, _, _| recalculate(s))
        .with_name(WAGES_FIELD)
        .fixed_width(15);

    LinearLayout::vertical()
        .child(description)
        .child(DummyView.fixed_height(1))
        .child(labeled_row(
            "Line 1-2. Net SE Income (Sched C/F):",
            se_income_field,
        ))
        .child(labeled_row("Line 1b.  CRP Payments:", crp_field))
        .child(DummyView.fixed_height(1))
        .child(labeled_row(
            "Line 6.   Wages Subject to SS Tax:",
            wages_field,
        ))
}

/// Create a labeled row with consistent formatting.
fn labeled_row<V: cursive::View>(label: &str, field: V) -> LinearLayout {
    LinearLayout::horizontal()
        .child(TextView::new(format!("{:38}", label)))
        .child(TextView::new("$ "))
        .child(field)
}

/// Parse a decimal value from an EditView field.
fn get_field_value(siv: &mut Cursive, name: &str) -> Decimal {
    siv.call_on_name(name, |view: &mut EditView| {
        let content = view.get_content();
        let cleaned: String = content.chars().filter(|c| *c != ',' && *c != ' ').collect();
        Decimal::from_str(&cleaned).unwrap_or(Decimal::ZERO)
    })
    .unwrap_or(Decimal::ZERO)
}

/// Format an optional decimal for input field display.
fn format_input(value: Option<Decimal>) -> String {
    match value {
        Some(d) if d != Decimal::ZERO => format!("{:.2}", d),
        _ => String::new(),
    }
}

/// Recalculate SE tax based on current input values.
fn recalculate(siv: &mut Cursive) {
    let se_income = get_field_value(siv, SE_INCOME_FIELD);
    let crp = get_field_value(siv, CRP_PAYMENTS_FIELD);
    let wages = get_field_value(siv, WAGES_FIELD);

    let config = get_se_config(siv);
    let worksheet = SeWorksheet::new(config);

    let result = worksheet.calculate(se_income, crp, wages).ok();

    siv.call_on_name(RESULTS_VIEW, |view: &mut TextView| {
        view.set_content(format_results(result.as_ref()));
    });
}

/// Format the calculation results for display.
fn format_results(result: Option<&SeWorksheetResult>) -> String {
    match result {
        Some(r) if r.below_threshold => {
            "Net SE earnings are below $400 threshold.\n\
             No self-employment tax is due.\n\n\
             You may skip this worksheet and proceed to\n\
             the Estimated Tax worksheet."
                .to_string()
        }
        Some(r) => {
            format!(
                "{:42} {:>12}\n\
                 {:42} {:>12}\n\
                 {:42} {:>12}\n\
                 {:42} {:>12}\n\
                 \n\
                 {:42} {:>12}\n\
                 {:42} {:>12}",
                "Line 3.  Net Earnings from SE:",
                format_currency(r.net_earnings),
                "Line 4.  Medicare Tax (2.9%):",
                format_currency(r.medicare_tax),
                "Line 8.  SS Taxable Earnings:",
                format_currency(r.ss_taxable_earnings),
                "Line 9.  Social Security Tax (12.4%):",
                format_currency(r.social_security_tax),
                "Line 10. SELF-EMPLOYMENT TAX:",
                format_currency(r.self_employment_tax),
                "Line 11. SE Tax Deduction (50%):",
                format_currency(r.se_tax_deduction),
            )
        }
        None => "Enter values above to see calculations.\n\n\
                 Results update automatically as you type."
            .to_string(),
    }
}

/// Format a decimal as currency.
fn format_currency(value: Decimal) -> String {
    // Simple formatting - could enhance with thousand separators
    format!("${:.2}", value)
}

/// Get SE worksheet configuration for the current tax year.
fn get_se_config(siv: &mut Cursive) -> SeWorksheetConfig {
    let tax_year = siv
        .with_user_data(|state: &mut AppState| state.tax_year)
        .unwrap_or(2025);

    // Tax year specific values
    let ss_wage_max = match tax_year {
        2024 => dec!(168600.00),
        2025 => dec!(176100.00),
        _ => dec!(176100.00), // Default to most recent
    };

    SeWorksheetConfig {
        ss_wage_max,
        ss_tax_rate: dec!(0.124),
        medicare_tax_rate: dec!(0.029),
        net_earnings_factor: dec!(0.9235),
        deduction_factor: dec!(0.50),
        min_se_threshold: dec!(400.00),
    }
}

/// Save the worksheet data to application state.
fn save_worksheet(siv: &mut Cursive) {
    let se_income = get_field_value(siv, SE_INCOME_FIELD);
    let crp = get_field_value(siv, CRP_PAYMENTS_FIELD);
    let wages = get_field_value(siv, WAGES_FIELD);

    let config = get_se_config(siv);
    let worksheet = SeWorksheet::new(config);

    match worksheet.calculate(se_income, crp, wages) {
        Ok(result) => {
            // Store in application state
            siv.with_user_data(|state: &mut AppState| {
                state.se_income = Some(se_income);
                state.crp_payments = Some(crp);
                state.wages = Some(wages);
                state.se_result = Some(result.clone());
            });

            // Close the form
            siv.pop_layer();

            // Show confirmation with summary
            let msg = if result.below_threshold {
                "SE income below threshold - no SE tax due.\n\n\
                 Data saved. Ready for Estimated Tax worksheet."
                    .to_string()
            } else {
                format!(
                    "Self-Employment Tax: {}\n\
                     SE Tax Deduction:    {}\n\n\
                     Data saved. Ready for Estimated Tax worksheet.",
                    format_currency(result.self_employment_tax),
                    format_currency(result.se_tax_deduction),
                )
            };

            siv.add_layer(
                Dialog::text(msg)
                    .title("SE Worksheet Saved")
                    .button("Continue", |s| {
                        s.pop_layer();
                        // Future: automatically proceed to 1040-ES worksheet
                    }),
            );
        }
        Err(e) => {
            siv.add_layer(
                Dialog::text(format!("Calculation error: {}", e))
                    .title("Error")
                    .button("OK", |s| {
                        s.pop_layer();
                    }),
            );
        }
    }
}
