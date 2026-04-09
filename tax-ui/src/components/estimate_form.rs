use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, RenderOnce, SharedString,
    Styled, Window, div, px,
};
use gpui::{Div, InteractiveElement};
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
use tax_core::calculations::{
    EstimatedTaxWorksheet, EstimatedTaxWorksheetContext, EstimatedTaxWorksheetResult,
};
use tax_core::{FilingStatusCode, TaxEstimateInput, TaxYearConfig};

use crate::app::FilingStatusData;
use crate::components::ErrorDialog;
use crate::models::SeWorksheetModel;
use crate::{
    components::{
        SeWorksheetForm, make_button, make_decimal_input, make_header_row, make_input_row,
        make_integer_input, make_select_row,
    },
    repository::ActiveTaxYear,
    utils::{parse_decimal, parse_optional_decimal},
};

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
}

impl EstimatedIncomeForm {
    pub fn new(
        worksheet: Entity<SeWorksheetForm>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let statuses = vec![
            SharedString::from("Single"),
            SharedString::from("Married Filing Jointly"),
            SharedString::from("Married Filing Separately"),
            SharedString::from("Head of Household"),
            SharedString::from("Qualifying Surviving Spouse"),
        ];

        let initial_index = statuses
            .iter()
            .position(|s| s.as_ref() == "Single")
            .map(|i| IndexPath::default().row(i));

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
                if let Ok(year) = raw.trim().parse::<i32>()
                    && is_loadable_tax_year(year)
                {
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

        let filing_status = cx.new(|cx| SelectState::new(statuses, initial_index, window, cx));
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
            Some(s) => match s.as_ref().try_into() {
                Ok(id) => Some(id),
                Err(e) => {
                    errors.push(format!("Filing status: {e}"));
                    None
                }
            },
        };

        let tax_year_value = self.tax_year.read(cx).value();
        let tax_year_s = tax_year_value.trim();
        let tax_year = if tax_year_s.is_empty() {
            errors.push("Tax year is required".to_string());
            None
        } else {
            match tax_year_s.parse::<i32>() {
                Ok(y) => Some(y),
                Err(e) => {
                    errors.push(format!("Tax year must be a number (e.g. 2025): {e}"));
                    None
                }
            }
        };

        let expected_agi = match parse_decimal(self.expected_agi.read(cx).value().as_str()) {
            Ok(d) => Some(d),
            Err(e) => {
                errors.push(format!("Expected AGI: {e}"));
                None
            }
        };
        let expected_deduction =
            match parse_decimal(self.expected_deduction.read(cx).value().as_str()) {
                Ok(d) => Some(d),
                Err(e) => {
                    errors.push(format!("Expected deduction: {e}"));
                    None
                }
            };

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
            expected_qbi_deduction: parse_optional_decimal(
                self.expected_qbi_deduction.read(cx).value().as_str(),
            ),
            expected_amt: parse_optional_decimal(self.expected_amt.read(cx).value().as_str()),
            expected_credits: parse_optional_decimal(
                self.expected_credits.read(cx).value().as_str(),
            ),
            expected_other_taxes: parse_optional_decimal(
                self.expected_other_taxes.read(cx).value().as_str(),
            ),
            expected_withholding: parse_optional_decimal(
                self.expected_withholding.read(cx).value().as_str(),
            ),
            prior_year_tax: parse_optional_decimal(self.prior_year_tax.read(cx).value().as_str()),
        };

        input.validate_for_submit()?;
        Ok(input)
    }

    /// Returns the raw tax year value, parsed if valid.
    pub fn tax_year(
        &self,
        cx: &App,
    ) -> Option<i32> {
        let value = self.tax_year.read(cx).value();
        value.trim().parse::<i32>().ok()
    }

    fn input_from_form_or_show_errors(
        &self,
        se_model: &SeWorksheetModel,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<TaxEstimateInput> {
        match self.to_input(se_model, cx) {
            Ok(input) => Some(input),
            Err(errors) => {
                for e in &errors {
                    tracing::warn!(%e, "form error");
                }
                ErrorDialog::show("Validation failed", &errors, window, cx);
                None
            }
        }
    }

    fn call_calculate_tax_estimate(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let se_model = self.worksheet.read(cx).get_se_model().clone();
        let Some(form_input) = self.input_from_form_or_show_errors(&se_model, window, cx) else {
            return;
        };

        let Some(tax_year_data) = ActiveTaxYear::get(cx).tax_year_data.clone() else {
            tracing::warn!("No tax year loaded; cannot calculate SE tax");
            // TODO: Add call to ErrorDialog
            return;
        };

        let config: &TaxYearConfig = &tax_year_data.config;
        let filing_status: FilingStatusCode = form_input.filing_status;
        let filing_data: &Vec<FilingStatusData> = &tax_year_data.statuses;

        let Some(filing_status_data) = filing_data
            .iter()
            .find(|f: &&FilingStatusData| f.filing_status.status_code == filing_status)
        else {
            // TODO: Handle this error situation
            return;
        };

        let worksheet_context = EstimatedTaxWorksheetContext {
            self_employment_tax: se_model.line_10_total_se_tax.unwrap_or_default(),
            refundable_credits: Decimal::ZERO,
            is_farmer_or_fisher: false,
            required_payment_threshold: config.req_pmnt_threshold,
        };
        let inputs = form_input.to_estimated_tax_worksheet_input(&worksheet_context);

        let tax_worksheet: EstimatedTaxWorksheet =
            EstimatedTaxWorksheet::new(&filing_status_data.tax_brackets);
        let result: EstimatedTaxWorksheetResult = match tax_worksheet.calculate(&inputs) {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(%error, "Estimated tax calculation failed");
                ErrorDialog::show("Calculation failed", &[error.to_string()], window, cx);
                return;
            }
        };

        tracing::info!(input = %form_input, %result, "Estimated taxes");
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
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.render_side_base()
            // .child(make_header_row("SE Worksheet Inputs:"))
            // .child(make_input_row(&self.se_income, "SE income: $"))
            // .child(make_input_row(
            //     &self.expected_crp_payments,
            //     "CRP payments: $",
            // ))
            // .child(make_input_row(&self.expected_wages, "Wages: $"))
            .child(make_header_row("1040-ES Worksheet Inputs:"))
            .child(make_input_row(&self.expected_agi, "Expected AGI: $"))
            .child(make_input_row(
                &self.expected_deduction,
                "Exp. deduction: $",
            ))
            .child(make_input_row(
                &self.expected_qbi_deduction,
                "QBI deduction: $",
            ))
            .child(make_input_row(&self.expected_amt, "AMT: $"))
            .child(make_input_row(&self.expected_credits, "Credits: $"))
            .child(make_input_row(&self.expected_other_taxes, "Other taxes: $"))
            .child(make_input_row(&self.expected_withholding, "Withholding: $"))
            .child(make_input_row(&self.prior_year_tax, "Prior year tax: $"))
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
            .child(self.render_toolbar(cx))
    }
}

fn is_loadable_tax_year(year: i32) -> bool {
    (1900..=2200).contains(&year)
}

fn tax_year_is_ready(
    tax_year_input: &str,
    active_tax_year: &ActiveTaxYear,
) -> bool {
    let trimmed = tax_year_input.trim();
    let Ok(year) = trimmed.parse::<i32>() else {
        return false;
    };

    is_loadable_tax_year(year)
        && active_tax_year.year == Some(year)
        && active_tax_year.tax_year_data.is_some()
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::app::TaxYearData;

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
}
