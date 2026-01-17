//! Self-Employment Tax worksheet view.
//!
//! This implements the SE Tax and Deduction Worksheet from Form 1040-ES,
//! allowing users to input self-employment income and see calculated
//! SE tax in real-time. Designed to fit in an 80x24 terminal.

use cursive::view::{Nameable, Resizable};
use cursive::views::{Dialog, DummyView, EditView, LinearLayout, TextView};
use cursive::Cursive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use tax_core::calculations::{SeWorksheet, SeWorksheetConfig, SeWorksheetResult};

use super::estimate_workflow::show_estimate_workflow;
use super::status_bar::{build_status_bar, hints};
use crate::state::AppState;

// View names for accessing components
const SE_INCOME_FIELD: &str = "se_income";
const CRP_FIELD: &str = "crp";
const WAGES_FIELD: &str = "wages";
const RESULTS_VIEW: &str = "results";

/// Display the Self-Employment Tax worksheet form.
pub fn show_se_worksheet(siv: &mut Cursive) {
    // Load existing values from state if any
    let (se_income, crp, wages) = siv
        .with_user_data(|state: &mut AppState| {
            (state.se_income, state.crp_payments, state.wages)
        })
        .unwrap_or((None, None, None));

    let form = build_compact_form(se_income, crp, wages);

    let results = TextView::new(format_results(None))
        .with_name(RESULTS_VIEW)
        .fixed_height(6);

    let status = build_status_bar(&[hints::TAB, hints::SHIFT_TAB, hints::ESC, hints::CTRL_Q]);

    let layout = LinearLayout::vertical()
        .child(form)
        .child(TextView::new("─".repeat(50)))
        .child(results)
        .child(DummyView.fixed_height(1))
        .child(status);

    let dialog = Dialog::around(layout)
        .title("SE Tax Worksheet")
        .button("Cancel", on_cancel)
        .button("Save", on_save)
        .padding_lrtb(1, 1, 0, 0);

    siv.add_layer(dialog);

    // Trigger initial calculation
    recalculate(siv);
}

/// Build a compact form that fits in 80x24.
fn build_compact_form(
    se_income: Option<Decimal>,
    crp: Option<Decimal>,
    wages: Option<Decimal>,
) -> LinearLayout {
    let se_field = EditView::new()
        .content(format_decimal(se_income))
        .on_edit(|s, _, _| recalculate(s))
        .on_submit(|s, _| on_save(s))
        .with_name(SE_INCOME_FIELD)
        .fixed_width(14);

    let crp_field = EditView::new()
        .content(format_decimal(crp))
        .on_edit(|s, _, _| recalculate(s))
        .on_submit(|s, _| on_save(s))
        .with_name(CRP_FIELD)
        .fixed_width(14);

    let wages_field = EditView::new()
        .content(format_decimal(wages))
        .on_edit(|s, _, _| recalculate(s))
        .on_submit(|s, _| on_save(s))
        .with_name(WAGES_FIELD)
        .fixed_width(14);

    LinearLayout::vertical()
        .child(field_row("SE Income (Sched C/F):", se_field))
        .child(field_row("CRP Payments:", crp_field))
        .child(field_row("Wages (SS taxable):", wages_field))
}

/// Create a labeled field row.
fn field_row<V: cursive::View>(label: &str, field: V) -> LinearLayout {
    LinearLayout::horizontal()
        .child(TextView::new(format!("{:24} $ ", label)))
        .child(field)
}

/// Get decimal value from a named EditView.
fn get_field(siv: &mut Cursive, name: &str) -> Decimal {
    siv.call_on_name(name, |v: &mut EditView| {
        let s = v.get_content();
        let clean: String = s.chars().filter(|c| *c != ',' && *c != ' ').collect();
        Decimal::from_str(&clean).unwrap_or(Decimal::ZERO)
    })
    .unwrap_or(Decimal::ZERO)
}

/// Format decimal for display in input field.
fn format_decimal(val: Option<Decimal>) -> String {
    match val {
        Some(d) if d != Decimal::ZERO => format!("{:.2}", d),
        _ => String::new(),
    }
}

/// Recalculate and update results display.
fn recalculate(siv: &mut Cursive) {
    let se_income = get_field(siv, SE_INCOME_FIELD);
    let crp = get_field(siv, CRP_FIELD);
    let wages = get_field(siv, WAGES_FIELD);

    let config = get_config(siv);
    let worksheet = SeWorksheet::new(config);
    let result = worksheet.calculate(se_income, crp, wages).ok();

    siv.call_on_name(RESULTS_VIEW, |v: &mut TextView| {
        v.set_content(format_results(result.as_ref()));
    });
}

/// Format calculation results for display.
fn format_results(result: Option<&SeWorksheetResult>) -> String {
    match result {
        Some(r) if r.below_threshold => {
            "Income below $400 threshold - no SE tax due.\n\
             You may skip this worksheet."
                .to_string()
        }
        Some(r) => {
            format!(
                "Net Earnings:     {:>12}   Medicare Tax:  {:>12}\n\
                 SS Earnings:      {:>12}   SS Tax:        {:>12}\n\n\
                 SE TAX (Line 10): {:>12}   Deduction:     {:>12}",
                fmt_currency(r.net_earnings),
                fmt_currency(r.medicare_tax),
                fmt_currency(r.ss_taxable_earnings),
                fmt_currency(r.social_security_tax),
                fmt_currency(r.self_employment_tax),
                fmt_currency(r.se_tax_deduction),
            )
        }
        None => "Enter values above to calculate SE tax.".to_string(),
    }
}

/// Format decimal as currency string.
fn fmt_currency(val: Decimal) -> String {
    format!("${:.2}", val)
}

/// Get SE worksheet config for current tax year.
fn get_config(siv: &mut Cursive) -> SeWorksheetConfig {
    let year = siv
        .with_user_data(|s: &mut AppState| s.tax_year)
        .unwrap_or(2025);

    let ss_max = match year {
        2024 => dec!(168600.00),
        2025 => dec!(176100.00),
        _ => dec!(176100.00),
    };

    SeWorksheetConfig {
        ss_wage_max: ss_max,
        ss_tax_rate: dec!(0.124),
        medicare_tax_rate: dec!(0.029),
        net_earnings_factor: dec!(0.9235),
        deduction_factor: dec!(0.50),
        min_se_threshold: dec!(400.00),
    }
}

/// Handle cancel button - discard and return to workflow.
fn on_cancel(siv: &mut Cursive) {
    siv.pop_layer();
    show_estimate_workflow(siv);
}

/// Handle save button - store results and return to workflow.
fn on_save(siv: &mut Cursive) {
    let se_income = get_field(siv, SE_INCOME_FIELD);
    let crp = get_field(siv, CRP_FIELD);
    let wages = get_field(siv, WAGES_FIELD);

    let config = get_config(siv);
    let worksheet = SeWorksheet::new(config);

    match worksheet.calculate(se_income, crp, wages) {
        Ok(result) => {
            // Store in state
            siv.with_user_data(|state: &mut AppState| {
                state.se_income = Some(se_income);
                state.crp_payments = Some(crp);
                state.wages = Some(wages);
                state.se_result = Some(result);
            });

            siv.pop_layer();
            show_estimate_workflow(siv);
        }
        Err(e) => {
            siv.add_layer(
                Dialog::text(format!("Error: {}", e))
                    .title("Calculation Error")
                    .button("OK", |s| {
                        s.pop_layer();
                    }),
            );
        }
    }
}
