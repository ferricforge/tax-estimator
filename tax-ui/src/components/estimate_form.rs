use gpui::{
    App, AppContext, Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    RenderOnce, SharedString, Styled, Window, div, px,
};
use gpui_component::WindowExt;
use gpui_component::dialog::DialogButtonProps;
use gpui_component::{
    IndexPath, h_flex,
    input::{InputEvent, InputState},
    select::{Select, SelectState},
    v_flex,
};
use regex::Regex;
use rust_decimal::Decimal;
use tax_core::{FilingStatusCode, TaxEstimate, TaxEstimateComputed, TaxEstimateInput};

use crate::components::{
    ErrorDialog, ResultForm, SeWorksheetForm, make_button, make_decimal_input, make_header_row,
    make_input_row, make_input_row_with_help, make_integer_input, make_select_row,
    set_decimal_input, set_input_value, set_optional_decimal_input, show_err,
};
use crate::estimate::{
    EstimateError, calculate_estimate, computed_values, loaded_tax_year_data, save_tax_estimate,
};
use crate::instructions::{UiInstructionField, help_for_field};
use crate::models::SeWorksheetModel;
use crate::repository::{ActiveTaxYear, TaxRepo};
use crate::utils::{parse_decimal, parse_optional_decimal};

#[derive(Clone, Debug)]
pub struct EstimatedIncomeForm {
    worksheet: Entity<SeWorksheetForm>,
    tax_year: Entity<InputState>,
    filing_status: Entity<SelectState<Vec<SharedString>>>,
    // 1040-ES Worksheet inputs
    // Line 1: adjusted gross income you expect for the year (see form instructions).
    expected_agi: Entity<InputState>,
    // Line 2a: deductions.
    expected_deduction: Entity<InputState>,
    // Line 2b: qualified business income deduction, if applicable.
    expected_qbi_deduction: Entity<InputState>,
    // Line 5: alternative minimum tax from Form 6251.
    expected_amt: Entity<InputState>,
    // Line 7: credits (do not include withholding on this line).
    expected_credits: Entity<InputState>,
    // Line 10: other taxes (see worksheet instructions).
    expected_other_taxes: Entity<InputState>,
    // Line 13: income tax withheld and estimated to be withheld (including pensions,
    // annuities, certain deferred income, and Additional Medicare Tax withholding).
    expected_withholding: Entity<InputState>,
    // Line 12b: required annual payment based on prior year's tax (per worksheet instructions).
    prior_year_tax: Entity<InputState>,
    is_tax_year_ready: bool,
    results: Entity<ResultForm>,
}

impl EstimatedIncomeForm {
    pub fn new(
        worksheet: Entity<SeWorksheetForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tax_year = make_integer_input("Tax Year", window, cx);
        tax_year.update(cx, |input_state, cx| {
            if let Ok(pattern) = Regex::new(r"^\d{0,4}$") {
                input_state.set_pattern(pattern, window, cx);
            }
        });

        // React to edits: once a full 4-digit year is entered, fetch its config.
        cx.subscribe(&tax_year, |this, input, event, cx| {
            if let InputEvent::Change = event {
                this.is_tax_year_ready = false;
                let raw = input.read(cx).value();
                if let Some(year) = parse_loadable_tax_year(&raw) {
                    ActiveTaxYear::load(year, cx);
                }
                // Recompute immediately so a previously-loaded matching year
                // re-enables without waiting for a new global update.
                this.recompute_tax_year_ready(cx);
                cx.notify();
            }
        })
        .detach();

        cx.observe_global::<ActiveTaxYear>(|this, cx| {
            this.recompute_tax_year_ready(cx);
            cx.notify();
        })
        .detach();

        let filing_status = cx.new(|cx| {
            SelectState::new(
                filing_status_labels(),
                Some(filing_status_row(FilingStatusCode::Single)),
                window,
                cx,
            )
        });
        let results = cx.new(|_| ResultForm::default());

        Self {
            worksheet,
            tax_year,
            filing_status,
            expected_agi: make_decimal_input("Exp AGI", 2, window, cx),
            expected_deduction: make_decimal_input("Exp deduction", 2, window, cx),
            expected_qbi_deduction: make_decimal_input("Exp QBI deduction", 2, window, cx),
            expected_amt: make_decimal_input("Exp AMT", 2, window, cx),
            expected_credits: make_decimal_input("Exp tax credits", 2, window, cx),
            expected_other_taxes: make_decimal_input("Exp other taxes", 2, window, cx),
            expected_withholding: make_decimal_input("Exp inc tax withheld", 2, window, cx),
            prior_year_tax: make_decimal_input("Prior year tax liability", 2, window, cx),
            is_tax_year_ready: false,
            results,
        }
    }

    fn recompute_tax_year_ready(
        &mut self,
        cx: &App,
    ) {
        let tax_year = self.tax_year.read(cx).value();
        self.is_tax_year_ready = tax_year_is_ready(tax_year.as_ref(), ActiveTaxYear::get(cx));
    }

    /// Collects the current form values into a [`TaxEstimateInput`].
    ///
    /// Runs parse/required-field checks then business-rule validation. Returns
    /// all errors so the user can see every problem at once.
    pub fn to_input(
        &self,
        se_model: &SeWorksheetModel,
        cx: &App,
    ) -> Result<TaxEstimateInput, Vec<String>> {
        let mut errors = Vec::new();

        let filing_status = match self.filing_status.read(cx).selected_value() {
            None => {
                errors.push("No filing status selected".to_string());
                None
            }
            Some(label) => {
                let code = filing_status_from_label(label.as_ref());
                if code.is_none() {
                    errors.push(format!("Filing status: unknown selection \"{label}\""));
                }
                code
            }
        };

        let tax_year_text = self.tax_year.read(cx).value();
        let tax_year = match tax_year_text.trim() {
            "" => {
                errors.push("Tax year is required".to_string());
                None
            }
            text => match text.parse::<i32>() {
                Ok(y) => Some(y),
                Err(e) => {
                    errors.push(format!("Tax year must be a number (e.g. 2025): {e}"));
                    None
                }
            },
        };

        let expected_agi = required_decimal(&self.expected_agi, "Expected AGI", &mut errors, cx);
        let expected_deduction = required_decimal(
            &self.expected_deduction,
            "Expected deduction",
            &mut errors,
            cx,
        );

        if !errors.is_empty() {
            return Err(errors);
        }

        let (Some(tax_year), Some(filing_status), Some(expected_agi), Some(expected_deduction)) =
            (tax_year, filing_status, expected_agi, expected_deduction)
        else {
            return Err(vec!["Required estimate fields were missing".to_string()]);
        };

        let input = TaxEstimateInput {
            tax_year,
            filing_status,
            se_income: se_model.line_1a_expected_se_income,
            expected_crp_payments: se_model.line_1b_expected_crp_payments,
            expected_wages: se_model.line_6_expected_wages,
            expected_agi,
            expected_deduction,
            expected_qbi_deduction: optional_decimal(&self.expected_qbi_deduction, cx),
            expected_amt: optional_decimal(&self.expected_amt, cx),
            expected_credits: optional_decimal(&self.expected_credits, cx),
            expected_other_taxes: optional_decimal(&self.expected_other_taxes, cx),
            expected_withholding: optional_decimal(&self.expected_withholding, cx),
            prior_year_tax: optional_decimal(&self.prior_year_tax, cx),
        };

        input.validate_for_submit()?;
        Ok(input)
    }

    /// Returns the raw tax year value, parsed if valid.
    pub fn tax_year(
        &self,
        cx: &App,
    ) -> Option<i32> {
        parse_tax_year(&self.tax_year.read(cx).value())
    }

    /// Populates the form's tax year, filing status, SE worksheet fields,
    /// and the 1040-ES worksheet inputs from a previously saved
    /// [`TaxEstimate`]. When the estimate carries computed results, those
    /// are shown in the results panel; otherwise the panel is cleared.
    ///
    /// Triggers [`ActiveTaxYear::load`] so the tax-year config is fetched
    /// and the **SE Worksheet** button becomes enabled once the config
    /// arrives.
    pub fn populate_from_estimate(
        &mut self,
        estimate: &TaxEstimate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = &estimate.input;

        set_input_value(&self.tax_year, input.tax_year.to_string(), window, cx);
        if is_loadable_tax_year(input.tax_year) {
            ActiveTaxYear::load(input.tax_year, cx);
        }
        self.recompute_tax_year_ready(cx);

        self.select_filing_status(input.filing_status, window, cx);

        set_decimal_input(&self.expected_agi, input.expected_agi, window, cx);
        set_decimal_input(
            &self.expected_deduction,
            input.expected_deduction,
            window,
            cx,
        );
        set_optional_decimal_input(
            &self.expected_qbi_deduction,
            input.expected_qbi_deduction,
            window,
            cx,
        );
        set_optional_decimal_input(&self.expected_amt, input.expected_amt, window, cx);
        set_optional_decimal_input(&self.expected_credits, input.expected_credits, window, cx);
        set_optional_decimal_input(
            &self.expected_other_taxes,
            input.expected_other_taxes,
            window,
            cx,
        );
        set_optional_decimal_input(
            &self.expected_withholding,
            input.expected_withholding,
            window,
            cx,
        );
        set_optional_decimal_input(&self.prior_year_tax, input.prior_year_tax, window, cx);

        self.results.update(cx, |rf, rf_cx| {
            if let Some(ref computed) = estimate.computed {
                rf.set_from_computed(computed);
            } else {
                rf.clear();
            }
            rf_cx.notify();
        });

        self.worksheet.update(cx, |ws, ws_cx| {
            ws.populate_from_estimate(input, window, ws_cx);
        });

        cx.notify();
    }

    /// Returns the form to its initial empty state: blank inputs, the default
    /// filing status, no results, and a fresh SE worksheet. Called after the
    /// active database changes so nothing from the previous project lingers.
    pub fn reset(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        set_input_value(&self.tax_year, "", window, cx);
        self.select_filing_status(FilingStatusCode::Single, window, cx);

        for input in [
            &self.expected_agi,
            &self.expected_deduction,
            &self.expected_qbi_deduction,
            &self.expected_amt,
            &self.expected_credits,
            &self.expected_other_taxes,
            &self.expected_withholding,
            &self.prior_year_tax,
        ] {
            set_input_value(input, "", window, cx);
        }

        self.results.update(cx, |results, results_cx| {
            results.clear();
            results_cx.notify();
        });

        // Rebuild the child worksheet rather than clearing it field-by-field:
        // only this form holds a handle to it, so swapping the entity is the
        // simplest way to reset every SE-worksheet input as well.
        self.worksheet = cx.new(|worksheet_cx| SeWorksheetForm::new(window, worksheet_cx));
        self.is_tax_year_ready = false;
        cx.notify();
    }

    /// Moves the filing-status dropdown to the row for `code`.
    fn select_filing_status(
        &self,
        code: FilingStatusCode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.filing_status.update(cx, |state, is_cx| {
            state.set_selected_index(Some(filing_status_row(code)), window, is_cx);
        });
    }

    fn call_calculate_tax_estimate(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Read everything needed from the worksheet while it is borrowed, so
        // the model does not have to be cloned.
        let (form_input, se_tax) = {
            let se_model = self.worksheet.read(cx).get_se_model();
            (
                self.to_input(se_model, cx),
                se_model.line_10_total_se_tax.unwrap_or_default(),
            )
        };
        let form_input = match form_input {
            Ok(input) => input,
            Err(errors) => {
                for e in &errors {
                    tracing::warn!(%e, "form error");
                }
                ErrorDialog::show("Validation failed", &errors, window, cx);
                return;
            }
        };

        let result = match calculate_estimate(ActiveTaxYear::get(cx), &form_input, se_tax) {
            Ok(result) => result,
            Err(error) => {
                show_estimate_error(&error, window, cx);
                return;
            }
        };

        // One summary feeds both the results panel and the saved record, so
        // the field mapping runs exactly once.
        let computed = computed_values(se_tax, &result);
        self.results.update(cx, |rf, cx| {
            rf.set_from_computed(&computed);
            cx.notify();
        });
        cx.notify();

        tracing::info!(input = %form_input, %result, "Estimated taxes");

        self.spawn_save_estimate(form_input, computed, window, cx);
    }

    /// Persists a calculated estimate in the background, reporting any
    /// failure in a dialog. The results are already on screen when this runs,
    /// so a failure here is a save failure, not a calculation failure.
    fn spawn_save_estimate(
        &self,
        form_input: TaxEstimateInput,
        computed: TaxEstimateComputed,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        const SAVE_FAILED_TITLE: &str = "Estimate not saved";
        const SAVE_FAILED_CONTEXT: &str =
            "The estimate was calculated but could not be saved to the database";

        let window_handle = window.window_handle();
        cx.spawn(async move |_this, async_cx| {
            let repo = match async_cx.update(|app_cx: &mut App| {
                TaxRepo::try_get(app_cx)
                    .map(|tax_repo| tax_repo.tax_repository_arc())
                    .ok_or_else(|| anyhow::anyhow!("The database connection is not available"))
            }) {
                Ok(Ok(repo)) => repo,
                Ok(Err(e)) | Err(e) => {
                    let e = e.context(SAVE_FAILED_CONTEXT);
                    tracing::warn!(error = ?e, "Cannot save tax estimate");
                    show_err(window_handle, async_cx, SAVE_FAILED_TITLE, &e);
                    return;
                }
            };

            if let Err(e) = save_tax_estimate(repo.as_ref(), &form_input, computed).await {
                let e = e.context(SAVE_FAILED_CONTEXT);
                tracing::error!(error = ?e, "save_tax_estimate failed");
                show_err(window_handle, async_cx, SAVE_FAILED_TITLE, &e);
            }
        })
        .detach();
    }

    fn call_se_worksheet_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tax_year = self.tax_year(cx);
        self.worksheet.update(cx, |ws, _cx| {
            ws.set_tax_year(tax_year);
        });

        let worksheet_for_dialog = self.worksheet.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .overlay_closable(false)
                .w(px(600.0))
                .margin_top(px(-20.0))
                .title("SE Tax Worksheet")
                .child(worksheet_for_dialog.clone())
                .button_props(DialogButtonProps::default().cancel_text("Close"))
                .footer(|_ok, cancel, window, cx| vec![cancel(window, cx)])
        });
    }

    fn render_results(
        &self,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.results.read(cx).has_results() {
            div().child(self.results.clone())
        } else {
            div()
        }
    }

    fn render_toolbar(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .id("window-body")
            .p_1()
            .gap_4()
            .items_center()
            .justify_center()
            .child(make_button(
                "calculate-estimates",
                "Calculate SE Tax",
                true,
                cx.listener(|this, _click_event, window, cx| {
                    this.call_calculate_tax_estimate(window, cx);
                }),
            ))
            .child(make_button(
                "open-se-worksheet",
                "SE Worksheet",
                self.is_tax_year_ready,
                cx.listener(|this, _ev, window, cx| {
                    this.call_se_worksheet_dialog(window, cx);
                }),
            ))
    }

    fn render_side_base(&self) -> Div {
        v_flex().gap_2().size_full()
    }

    fn render_left_side(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.render_side_base()
            .child(make_header_row("Year and Filing Status"))
            .child(make_input_row(&self.tax_year, "Tax Year"))
            .child(make_select_row(
                "Filing Status:",
                Select::new(&self.filing_status).w_full().render(window, cx),
            ))
    }

    fn render_right_side(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected_year = self.tax_year(cx);

        self.render_side_base()
            .child(make_header_row("1040-ES Worksheet Inputs:"))
            .child(make_input_row_with_help(
                &self.expected_agi,
                "Expected AGI: $",
                help_for_field(UiInstructionField::ExpectedAgi, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_deduction,
                "Exp. deduction: $",
                help_for_field(UiInstructionField::ExpectedDeduction, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_qbi_deduction,
                "QBI deduction: $",
                help_for_field(UiInstructionField::ExpectedQbiDeduction, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_amt,
                "AMT: $",
                help_for_field(UiInstructionField::ExpectedAmt, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_credits,
                "Credits: $",
                help_for_field(UiInstructionField::ExpectedCredits, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_other_taxes,
                "Other taxes: $",
                help_for_field(UiInstructionField::ExpectedOtherTaxes, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.expected_withholding,
                "Withholding: $",
                help_for_field(UiInstructionField::ExpectedWithholding, selected_year),
            ))
            .child(make_input_row_with_help(
                &self.prior_year_tax,
                "Prior year tax: $",
                help_for_field(UiInstructionField::PriorYearTax, selected_year),
            ))
    }
}

impl Render for EstimatedIncomeForm {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .child(
                div().w_full().child(
                    h_flex()
                        .items_start()
                        .gap_2()
                        .w_full()
                        .child(self.render_left_side(window, cx))
                        .child(self.render_right_side(window, cx)),
                ),
            )
            .child(self.render_results(cx))
            .child(self.render_toolbar(cx))
    }
}

// ---------------------------------------------------------------------------
// Filing status dropdown
// ---------------------------------------------------------------------------

/// Dropdown entries for the filing-status select, in display order.
///
/// The label is both what the user sees and what `SelectState` reports as the
/// selected value, so every code-to-label and label-to-code lookup goes
/// through this table.
const FILING_STATUS_OPTIONS: [(FilingStatusCode, &str); 5] = [
    (FilingStatusCode::Single, "Single"),
    (
        FilingStatusCode::MarriedFilingJointly,
        "Married Filing Jointly",
    ),
    (
        FilingStatusCode::MarriedFilingSeparately,
        "Married Filing Separately",
    ),
    (FilingStatusCode::HeadOfHousehold, "Head of Household"),
    (
        FilingStatusCode::QualifyingSurvivingSpouse,
        "Qualifying Surviving Spouse",
    ),
];

/// The dropdown labels, in display order.
fn filing_status_labels() -> Vec<SharedString> {
    FILING_STATUS_OPTIONS
        .iter()
        .map(|(_, label)| SharedString::from(*label))
        .collect()
}

/// Position of `code` in the dropdown.
fn filing_status_index(code: FilingStatusCode) -> usize {
    FILING_STATUS_OPTIONS
        .iter()
        .position(|(candidate, _)| *candidate == code)
        .expect("every FilingStatusCode has an entry in FILING_STATUS_OPTIONS")
}

/// The dropdown row for `code`.
fn filing_status_row(code: FilingStatusCode) -> IndexPath {
    IndexPath::default().row(filing_status_index(code))
}

/// The label shown in the dropdown for `code`.
fn filing_status_label(code: FilingStatusCode) -> &'static str {
    FILING_STATUS_OPTIONS[filing_status_index(code)].1
}

/// The code for a dropdown label; `None` for text that is not in the table.
fn filing_status_from_label(label: &str) -> Option<FilingStatusCode> {
    FILING_STATUS_OPTIONS
        .iter()
        .find(|(_, candidate)| *candidate == label)
        .map(|(code, _)| *code)
}

// ---------------------------------------------------------------------------
// Tax year
// ---------------------------------------------------------------------------

fn is_loadable_tax_year(year: i32) -> bool {
    (1900..=2200).contains(&year)
}

/// Parses the tax-year text, ignoring surrounding whitespace.
fn parse_tax_year(raw: &str) -> Option<i32> {
    raw.trim().parse::<i32>().ok()
}

/// Like [`parse_tax_year`], but only for years a configuration can be loaded
/// for.
fn parse_loadable_tax_year(raw: &str) -> Option<i32> {
    parse_tax_year(raw).filter(|year| is_loadable_tax_year(*year))
}

fn tax_year_is_ready(
    tax_year_input: &str,
    active_tax_year: &ActiveTaxYear,
) -> bool {
    parse_loadable_tax_year(tax_year_input)
        .is_some_and(|year| loaded_tax_year_data(active_tax_year, year).is_some())
}

// ---------------------------------------------------------------------------
// Field parsing
// ---------------------------------------------------------------------------

/// Parses a required currency field, recording a labelled error when the text
/// is not a valid number.
fn required_decimal(
    input: &Entity<InputState>,
    label: &str,
    errors: &mut Vec<String>,
    cx: &App,
) -> Option<Decimal> {
    match parse_decimal(input.read(cx).value().as_str()) {
        Ok(value) => Some(value),
        Err(e) => {
            errors.push(format!("{label}: {e}"));
            None
        }
    }
}

/// Parses an optional currency field; blank text yields `None`.
fn optional_decimal(
    input: &Entity<InputState>,
    cx: &App,
) -> Option<Decimal> {
    parse_optional_decimal(input.read(cx).value().as_str())
}

// ---------------------------------------------------------------------------
// Error reporting
// ---------------------------------------------------------------------------

/// Logs an [`EstimateError`] and shows the matching dialog.
fn show_estimate_error(
    error: &EstimateError,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, message) = match error {
        EstimateError::TaxYearNotLoaded { year } => {
            tracing::warn!(
                requested = *year,
                active = ?ActiveTaxYear::get(cx).year,
                "No matching tax year loaded; cannot calculate"
            );
            (
                "Tax year not loaded",
                format!(
                    "Tax data for {year} is not loaded. It may still be loading, or no \
                     configuration exists for that year. Check the tax year and try again."
                ),
            )
        }
        EstimateError::MissingFilingStatus { year, status } => {
            tracing::error!(
                ?status,
                tax_year = *year,
                "Filing status data missing from tax year configuration"
            );
            (
                "Missing tax data",
                format!(
                    "No tax bracket data was found for filing status \"{}\" in tax year {year}. \
                     The configuration for this year appears to be incomplete.",
                    filing_status_label(*status)
                ),
            )
        }
        EstimateError::Worksheet(error) => {
            tracing::warn!(error = ?error, "Estimated tax calculation failed");
            // `{:#}` keeps the full anyhow chain (context + worksheet error).
            ("Calculation failed", format!("{error:#}"))
        }
    };

    ErrorDialog::show(title, &[message], window, cx);
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal::Decimal;
    use tax_core::TaxYearConfig;

    use crate::models::TaxYearData;

    use super::*;

    fn active_tax_year(
        year: Option<i32>,
        has_config: bool,
    ) -> ActiveTaxYear {
        ActiveTaxYear {
            year,
            tax_year_data: has_config.then(|| TaxYearData {
                config: TaxYearConfig {
                    tax_year: year.unwrap_or_default(),
                    ss_wage_max: Decimal::ZERO,
                    ss_tax_rate: Decimal::ZERO,
                    medicare_tax_rate: Decimal::ZERO,
                    se_tax_deduct_pcnt: Decimal::ZERO,
                    se_deduction_factor: Decimal::ZERO,
                    req_pmnt_threshold: Decimal::ZERO,
                    min_se_threshold: Decimal::ZERO,
                },
                statuses: Vec::new(),
            }),
        }
    }

    #[test]
    fn tax_year_ready_when_year_matches_and_config_present() {
        let active = active_tax_year(Some(2025), true);
        assert!(tax_year_is_ready("2025", &active));
    }

    #[test]
    fn tax_year_not_ready_when_input_empty() {
        let active = active_tax_year(Some(2025), true);
        assert!(!tax_year_is_ready("", &active));
        assert!(!tax_year_is_ready("   ", &active));
    }

    #[test]
    fn tax_year_not_ready_when_input_invalid() {
        let active = active_tax_year(Some(2025), true);
        assert!(!tax_year_is_ready("abcd", &active));
    }

    #[test]
    fn tax_year_not_ready_when_out_of_range() {
        let active = active_tax_year(Some(1899), true);
        assert!(!tax_year_is_ready("1899", &active));
    }

    #[test]
    fn tax_year_not_ready_when_active_year_differs() {
        let active = active_tax_year(Some(2024), true);
        assert!(!tax_year_is_ready("2025", &active));
    }

    #[test]
    fn tax_year_not_ready_when_config_missing() {
        let active = active_tax_year(Some(2025), false);
        assert!(!tax_year_is_ready("2025", &active));
    }

    #[test]
    fn parse_tax_year_trims_whitespace_and_rejects_non_numbers() {
        assert_eq!(parse_tax_year(" 2025 "), Some(2025));
        assert_eq!(parse_tax_year(""), None);
        assert_eq!(parse_tax_year("20x5"), None);
    }

    #[test]
    fn parse_loadable_tax_year_rejects_years_outside_range() {
        assert_eq!(parse_loadable_tax_year("2025"), Some(2025));
        assert_eq!(parse_loadable_tax_year("1899"), None);
        assert_eq!(parse_loadable_tax_year("2201"), None);
    }

    #[test]
    fn filing_status_table_round_trips_every_entry() {
        for (index, (code, label)) in FILING_STATUS_OPTIONS.iter().enumerate() {
            assert_eq!(filing_status_index(*code), index);
            assert_eq!(filing_status_label(*code), *label);
            assert_eq!(
                filing_status_from_label(label).map(filing_status_index),
                Some(index)
            );
        }
        assert_eq!(filing_status_from_label("Not a status"), None);
    }
}
