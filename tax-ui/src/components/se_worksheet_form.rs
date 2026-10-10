use anyhow::Result;
use gpui::{
    App, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window,
};
use gpui_component::{
    h_flex,
    input::{InputEvent, InputState},
    v_flex,
};
use rust_decimal::Decimal;
use tax_core::calculations::SeWorksheetResult;
use tax_core::validation::{
    SeField, SeRawInputs, SeWorksheetInputs, SeWorksheetValidator, Validated, Worksheet,
};
use tax_core::{TaxEstimateInput, TaxYearConfig};

use crate::estimate::se_tax_estimate;
use crate::{
    components::{
        FieldVisibility, confirm_clear, field_messages, form_error_banner, make_button,
        make_decimal_input, make_display_row_with_help, make_header_row,
        make_input_row_fixed_with_help, set_input_value, set_optional_decimal_input,
        visible_issues,
    },
    instructions::{UiInstructionField, help_for_field},
    models::SeWorksheetModel,
    state::ActiveTaxYear,
};

/// Key that remembers the "Don't show this again" choice for this worksheet.
const CLEAR_CONFIRM_KEY: &str = "se-worksheet-clear";

/// Explains what clearing the SE worksheet does to the saved estimate.
const CLEAR_CONFIRM_MESSAGE: &str = "Clearing the SE worksheet removes its figures from this \
    estimate. The next time you save the estimate, the saved SE income, CRP payments, and wages \
    are replaced with blanks.";

pub struct SeWorksheetForm {
    /// Line 1a: Expected SE income (Form 1040-ES).
    se_income: Entity<InputState>,
    /// Line 1b: Expected CRP payments.
    crp_payments: Entity<InputState>,
    /// Line 6: Expected wages (SS or tier 1 RRTA).
    expected_wages: Entity<InputState>,
    /// Full worksheet model (lines 1a–11).
    model: SeWorksheetModel,
    /// Result of the most recent validation pass over every field.
    validated: Validated<SeWorksheetInputs, SeField>,
    /// Which fields show their messages.
    visibility: FieldVisibility<SeField>,
}

impl SeWorksheetForm {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let se_income = make_decimal_input("Net SE income", 2, window, cx);
        let crp_payments = make_decimal_input("CRP payments", 2, window, cx);
        let expected_wages = make_decimal_input("Expected wages", 2, window, cx);

        // Each edit marks its field as touched and re-checks every field, so
        // the stored result is never stale.
        for (input, field) in [
            (&se_income, SeField::SeIncome),
            (&crp_payments, SeField::CrpPayments),
            (&expected_wages, SeField::ExpectedWages),
        ] {
            cx.subscribe(input, move |this, _input, event, cx| {
                if let InputEvent::Change = event {
                    this.visibility.touch(field);
                    this.validate_inputs(cx);
                    cx.notify();
                }
            })
            .detach();
        }

        // Re-render whenever the active tax year's config changes.
        cx.observe_global::<ActiveTaxYear>(|this, cx| {
            this.model.line_5_ss_maximum_income = ActiveTaxYear::ss_wage_max(cx);
            if this.model.line_1a_expected_se_income.is_some()
                || this.model.line_1b_expected_crp_payments.is_some()
                || this.model.line_6_expected_wages.is_some()
            {
                this.recalculate_model(cx);
            }
            cx.notify();
        })
        .detach();

        Self {
            se_income,
            crp_payments,
            expected_wages,
            model: SeWorksheetModel::default(),
            validated: Validated::default(),
            visibility: FieldVisibility::default(),
        }
    }

    pub fn set_tax_year(
        &mut self,
        year: Option<i32>,
    ) {
        self.model.tax_year = year;
    }

    pub fn se_income(
        &self,
        cx: &App,
    ) -> SharedString {
        self.se_income.read(cx).value()
    }

    pub fn crp_payments(
        &self,
        cx: &App,
    ) -> SharedString {
        self.crp_payments.read(cx).value()
    }

    pub fn expected_wages(
        &self,
        cx: &App,
    ) -> SharedString {
        self.expected_wages.read(cx).value()
    }

    pub fn set_calculated_values(
        &mut self,
        values: SeWorksheetModel,
    ) {
        self.model = values;
    }

    /// Returns `true` when no field carries a blocking message.
    ///
    /// Callers that persist the worksheet should check this before writing.
    pub fn is_valid(&self) -> bool {
        self.validated.is_valid()
    }

    /// Populates the worksheet input fields and model from a saved estimate,
    /// then validates and runs the SE calculation so the form opens with all
    /// lines filled.
    ///
    /// Sets lines 1a, 1b, and 6 from the estimate's SE-related fields,
    /// computes line 2, and preserves line 5 from the active tax year.
    pub fn populate_from_estimate(
        &mut self,
        input: &TaxEstimateInput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preserved_line_5 = ActiveTaxYear::ss_wage_max(cx);

        self.model = SeWorksheetModel::default();
        self.model.line_5_ss_maximum_income = preserved_line_5;
        self.model.tax_year = Some(input.tax_year);
        self.model.line_1a_expected_se_income = input.se_income;
        self.model.line_1b_expected_crp_payments = input.expected_crp_payments;
        self.model.line_6_expected_wages = input.expected_wages;

        let income = input.se_income.unwrap_or(Decimal::ZERO);
        let crp = input.expected_crp_payments.unwrap_or(Decimal::ZERO);
        self.model.line_2_subtract_1b_from_1a = Some(income - crp);

        self.visibility.reset();
        set_optional_decimal_input(&self.se_income, input.se_income, window, cx);
        set_optional_decimal_input(&self.crp_payments, input.expected_crp_payments, window, cx);
        set_optional_decimal_input(&self.expected_wages, input.expected_wages, window, cx);

        // Saved records are validated as well, so a record written before the
        // rules existed reports its problems instead of failing silently.
        self.calculate_se(cx);
        cx.notify();
    }

    pub fn get_se_model(&self) -> &SeWorksheetModel {
        &self.model
    }

    /// Checks every field and stores the result.
    fn validate_inputs(
        &mut self,
        cx: &App,
    ) {
        let se_income = self.se_income.read(cx).value();
        let crp_payments = self.crp_payments.read(cx).value();
        let expected_wages = self.expected_wages.read(cx).value();

        let raw = SeRawInputs {
            se_income: se_income.as_str(),
            crp_payments: crp_payments.as_str(),
            expected_wages: expected_wages.as_str(),
        };

        self.validated = SeWorksheetValidator::validate(raw);
    }

    /// Copies the validated values into lines 1a, 1b, 6, and line 2 (1a − 1b).
    fn apply_inputs(
        &mut self,
        inputs: &SeWorksheetInputs,
    ) {
        self.model.line_1a_expected_se_income = inputs.se_income;
        self.model.line_1b_expected_crp_payments = inputs.crp_payments;
        self.model.line_6_expected_wages = inputs.expected_wages;

        let income = inputs.se_income.unwrap_or(Decimal::ZERO);
        let crp = inputs.crp_payments.unwrap_or(Decimal::ZERO);
        self.model.line_2_subtract_1b_from_1a = Some(income - crp);
    }

    /// Treats the worksheet as submitted, then calculates it when every field
    /// is valid. Returns `false` and leaves the model untouched otherwise.
    fn calculate_se(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        self.visibility.submit();
        self.validate_inputs(cx);

        if !self.is_valid() {
            tracing::warn!(
                errors = self.validated.report.error_count(),
                "SE worksheet inputs failed validation; calculation skipped"
            );
            return false;
        }

        let inputs = self.validated.inputs;
        self.apply_inputs(&inputs);
        self.recalculate_model(cx);
        true
    }

    /// Computes lines 3, 4, 7–11 from the current model using the active tax
    /// year's configuration. No-ops with a warning when no tax year is loaded.
    fn recalculate_model(
        &mut self,
        cx: &App,
    ) {
        let Some(tax_year_data) = ActiveTaxYear::get(cx).data() else {
            tracing::warn!("No tax year loaded; cannot calculate SE tax");
            return;
        };

        match make_se_estimate(&tax_year_data.config, &self.model) {
            Ok(result) => self.model.from_worksheet_result(&result),
            Err(e) => {
                tracing::warn!(%e, "Calculate SE Tax failed");
            }
        }
    }

    fn clear(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model = SeWorksheetModel::default();
        set_input_value(&self.se_income, "", window, cx);
        set_input_value(&self.expected_wages, "", window, cx);
        set_input_value(&self.crp_payments, "", window, cx);
        self.visibility.reset();
        self.validate_inputs(cx);
        self.model.line_5_ss_maximum_income = ActiveTaxYear::ss_wage_max(cx);
        cx.notify();
    }

    /// Renders the messages for one field, when its visibility allows them.
    fn messages_for(
        &self,
        field: SeField,
        cx: &App,
    ) -> Option<impl IntoElement> {
        let issues = visible_issues(&self.validated.report, &self.visibility, field);
        field_messages(issues, cx)
    }
}

impl Render for SeWorksheetForm {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();
        let selected_year = self.model.tax_year;
        let show_banner = self.visibility.is_submitted() && !self.is_valid();

        v_flex()
            .gap_2()
            .p_4()
            .child(make_header_row("SE Worksheet Inputs:"))
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.se_income,
                        "1a. Expected SE income: $",
                        help_for_field(UiInstructionField::SeIncome, selected_year),
                    ))
                    .children(self.messages_for(SeField::SeIncome, cx)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.crp_payments,
                        "1b. Expected CRP payments: $",
                        help_for_field(UiInstructionField::CrpPayments, selected_year),
                    ))
                    .children(self.messages_for(SeField::CrpPayments, cx)),
            )
            .child(make_display_row_with_help(
                "2. Subtract line 1b from line 1a:",
                self.model.line_2_subtract_1b_from_1a,
                help_for_field(UiInstructionField::SeLine2, selected_year),
            ))
            .child(make_display_row_with_help(
                "3. Multiply line 2 by 92.35% (0.9235):",
                self.model.line_3_net_earnings,
                help_for_field(UiInstructionField::SeLine3, selected_year),
            ))
            .child(make_display_row_with_help(
                "4. Multiply line 3 by 2.9% (0.029):",
                self.model.line_4_medicare_tax,
                help_for_field(UiInstructionField::SeLine4, selected_year),
            ))
            .child(make_display_row_with_help(
                "5. Social security tax maximum income:",
                self.model.line_5_ss_maximum_income,
                help_for_field(UiInstructionField::SeLine5, selected_year),
            ))
            .child(
                v_flex()
                    .gap_1()
                    .child(make_input_row_fixed_with_help(
                        &self.expected_wages,
                        "6. Expected wages (SS / tier 1 RRTA 6.2%): $",
                        help_for_field(UiInstructionField::ExpectedWages, selected_year),
                    ))
                    .children(self.messages_for(SeField::ExpectedWages, cx)),
            )
            .child(make_display_row_with_help(
                "7. Subtract line 6 from line 5:",
                self.model.line_7_remaining_ss_base,
                help_for_field(UiInstructionField::SeLine7, selected_year),
            ))
            .child(make_display_row_with_help(
                "8. Smaller of line 3 or line 7:",
                self.model.line_8_ss_taxable_earnings,
                help_for_field(UiInstructionField::SeLine8, selected_year),
            ))
            .child(make_display_row_with_help(
                "9. Multiply line 8 by 12.4% (0.124):",
                self.model.line_9_social_security_tax,
                help_for_field(UiInstructionField::SeLine9, selected_year),
            ))
            .child(make_display_row_with_help(
                "10. Add lines 4 and 9:",
                self.model.line_10_total_se_tax,
                help_for_field(UiInstructionField::SeLine10, selected_year),
            ))
            .child(make_display_row_with_help(
                "11. Multiply line 10 by 50% (0.50):",
                self.model.line_11_deductible_se_tax,
                help_for_field(UiInstructionField::SeLine11, selected_year),
            ))
            .children(show_banner.then(|| {
                form_error_banner("Correct the fields marked above before calculating.", cx)
            }))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .mt_4()
                    .child(make_button("calculate_se_tax", "Calculate", true, {
                        let this = this.clone();
                        move |_ev, _window, cx| {
                            this.update(
                                cx,
                                |form: &mut SeWorksheetForm,
                                 cx: &mut Context<'_, SeWorksheetForm>| {
                                    form.calculate_se(cx);
                                    cx.notify();
                                },
                            );
                        }
                    }))
                    .child(make_button(
                        "calculate_se_clear",
                        "Clear",
                        true,
                        move |_ev: &ClickEvent, window: &mut Window, app_cx: &mut App| {
                            let this = this.clone();
                            confirm_clear(
                                CLEAR_CONFIRM_KEY,
                                CLEAR_CONFIRM_MESSAGE,
                                move |window, app| {
                                    this.update(app, |form, cx| {
                                        form.clear(window, cx);
                                    });
                                },
                                window,
                                app_cx,
                            );
                        },
                    )),
            )
    }
}

fn make_se_estimate(
    config: &TaxYearConfig,
    model: &SeWorksheetModel,
) -> Result<SeWorksheetResult> {
    let se_income = model.line_1a_expected_se_income.unwrap_or_default();
    let crp_payments = model.line_1b_expected_crp_payments.unwrap_or_default();
    let wages = model.line_6_expected_wages.unwrap_or_default();

    se_tax_estimate(config, se_income, crp_payments, wages)
}
