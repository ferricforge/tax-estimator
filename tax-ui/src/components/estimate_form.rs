use gpui::{
    App, AppContext, ClickEvent, Context, Div, Entity, Hsla, IntoElement, ParentElement, Render,
    RenderOnce, SharedString, Styled, TextAlign, Window, div, px,
};
use gpui_component::{
    IndexPath, h_flex,
    input::{Input, InputState, MaskPattern},
    select::{Select, SelectState},
    v_flex,
};
use regex::Regex;

use crate::{
    components::make_button,
    models::EstimatedIncomeModel,
    utils::{parse_decimal, parse_optional_decimal},
};

#[derive(Clone, Debug)]
pub struct EstimatedIncomeForm {
    tax_year: Entity<InputState>,

    // User-provided values (1040-ES Worksheet inputs)
    filing_status: Entity<SelectState<Vec<SharedString>>>,
    expected_agi: Entity<InputState>,
    expected_deduction: Entity<InputState>,
    expected_qbi_deduction: Entity<InputState>,
    expected_amt: Entity<InputState>,
    expected_credits: Entity<InputState>,
    expected_other_taxes: Entity<InputState>,
    expected_withholding: Entity<InputState>,
    prior_year_tax: Entity<InputState>,

    // User-provided values (SE Worksheet inputs)
    se_income: Entity<InputState>,
    expected_crp_payments: Entity<InputState>,
    expected_wages: Entity<InputState>,
}

impl EstimatedIncomeForm {
    pub fn new(
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

        let tax_year = make_input_state_integer_mask("Tax Year", window, cx);
        tax_year.update(cx, |input_state, cx| {
            input_state.set_pattern(Regex::new(r"^\d{0,4}$").unwrap(), window, cx);
        });

        let filing_status = cx.new(|cx| SelectState::new(statuses, initial_index, window, cx));

        // SE Worksheet values
        let se_income = make_input_state_with_decimal_mask("Self-emp income", 2, window, cx);
        let expected_crp_payments =
            make_input_state_with_decimal_mask("Exp CRP payments", 2, window, cx);
        let expected_wages = make_input_state_with_decimal_mask("Exp wages", 2, window, cx);

        // 1040-ES Worksheet
        let expected_agi = make_input_state_with_decimal_mask("Exp AGI", 2, window, cx);
        let expected_deduction = make_input_state_with_decimal_mask("Exp deduction", 2, window, cx);
        let expected_qbi_deduction =
            make_input_state_with_decimal_mask("Exp QBI deduction", 2, window, cx);
        let expected_amt = make_input_state_with_decimal_mask("Exp AMT", 2, window, cx);
        let expected_credits = make_input_state_with_decimal_mask("Exp tax credits", 2, window, cx);
        let expected_other_taxes =
            make_input_state_with_decimal_mask("Exp other taxes", 2, window, cx);
        let expected_withholding =
            make_input_state_with_decimal_mask("Exp inc tax withheld", 2, window, cx);
        let prior_year_tax =
            make_input_state_with_decimal_mask("Prior year tax liability", 2, window, cx);

        Self {
            tax_year,
            filing_status,
            expected_agi,
            expected_deduction,
            expected_qbi_deduction,
            expected_amt,
            expected_credits,
            expected_other_taxes,
            expected_withholding,
            prior_year_tax,
            se_income,
            expected_crp_payments,
            expected_wages,
        }
    }

    /// Collects the current form values into an [`EstimatedIncomeModel`].
    ///
    /// Runs parse/required-field checks then business-rule validation. Returns all
    /// errors (parse or validation) so the user can see every problem at once.
    pub fn to_model(
        &self,
        cx: &App,
    ) -> Result<EstimatedIncomeModel, Vec<String>> {
        let mut errors = Vec::new();

        let filing_status_id = match self.filing_status.read(cx).selected_value() {
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

        let model = EstimatedIncomeModel {
            tax_year: tax_year.unwrap(),
            filing_status_id: filing_status_id.unwrap(),
            expected_agi: expected_agi.unwrap(),
            expected_deduction: expected_deduction.unwrap(),
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
            se_income: parse_optional_decimal(self.se_income.read(cx).value().as_str()),
            expected_crp_payments: parse_optional_decimal(
                self.expected_crp_payments.read(cx).value().as_str(),
            ),
            expected_wages: parse_optional_decimal(self.expected_wages.read(cx).value().as_str()),
        };

        if let Err(validation_errors) = model.validate_for_submit() {
            return Err(validation_errors);
        }
        Ok(model)
    }
}

impl Render for EstimatedIncomeForm {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .child(make_labeled_row("Enter expected values for the year:"))
            .child(
                h_flex()
                    .gap_2()
                    .size_full()
                    .child(
                        v_flex()
                            .gap_2()
                            .size_full()
                            .child(make_input_row(&self.tax_year, "Tax Year")),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .size_full()
                            .child(make_select_row(
                                "Filing Status:",
                                Select::new(&self.filing_status).w_full().render(window, cx),
                            ))
                            .child(make_header_row("SE Worksheet Inputs:"))
                            .child(make_input_row(&self.se_income, "SE income: $"))
                            .child(make_input_row(
                                &self.expected_crp_payments,
                                "CRP payments: $",
                            ))
                            .child(make_input_row(&self.expected_wages, "Wages: $"))
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
                            .child(make_input_row(&self.prior_year_tax, "Prior year tax: $")),
                    ),
            )
    }
}

fn make_input_state_with_decimal_mask(
    label: impl Into<SharedString>,
    decimals: usize,
    window: &mut Window,
    cx: &mut Context<EstimatedIncomeForm>,
) -> Entity<InputState> {
    let pattern: MaskPattern = MaskPattern::Number {
        separator: Some('_'),
        fraction: Some(decimals),
    };

    cx.new(|closure_cx| {
        InputState::new(window, closure_cx)
            //.text_align(TextAlign::Right)
            .mask_pattern(pattern)
            .placeholder(label.into())
            .multi_line(false)
    })
}

fn make_input_state_integer_mask(
    label: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<EstimatedIncomeForm>,
) -> Entity<InputState> {
    let pattern: MaskPattern = MaskPattern::Number {
        separator: None,
        fraction: Some(0),
    };

    cx.new(|closure_cx| {
        InputState::new(window, closure_cx)
            //.text_align(TextAlign::Right)
            .mask_pattern(pattern)
            .placeholder(label.into())
            .multi_line(false)
    })
}

#[allow(unused)]
fn make_input_row_with_button(
    state: &Entity<InputState>,
    input_label: impl Into<SharedString>,
    button_id: impl Into<SharedString>,
    button_label: impl Into<SharedString>,
    button_callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    make_labeled_row(input_label)
        .child(Input::new(state).flex_grow())
        .child(make_button(button_id, button_label, button_callback))
}

fn make_input_row(
    state: &Entity<InputState>,
    input_label: impl Into<SharedString>,
) -> Div {
    make_labeled_row(input_label).child(Input::new(state).flex_grow())
}

/// Creates a labeled row containing a text label and an already-rendered
/// [`Select`] dropdown, styled consistently with [`make_input_row`].
fn make_select_row(
    label: impl Into<SharedString>,
    select_element: impl IntoElement,
) -> Div {
    make_labeled_row(label).child(select_element)
}

/// Creates the common outer container and label used by both input and select
/// rows, ensuring consistent alignment, spacing, and border styling.
fn make_labeled_row(label: impl Into<SharedString>) -> Div {
    h_flex()
        .items_center()
        .gap_5()
        .p(px(2.))
        .rounded_md()
        .border_1()
        .child(
            div()
                .min_w(px(150.))
                .text_align(TextAlign::Right)
                .child(label.into()),
        )
}

/// Creates the common outer container and label used by both input and select
/// rows, ensuring consistent alignment, spacing, and border styling.
fn make_header_row(header: impl Into<SharedString>) -> Div {
    h_flex()
        .items_center()
        .gap_5()
        .p(px(2.))
        .rounded_md()
        .child(
            div()
                .size_full()
                .border_1()
                .gap_1()
                .p_1()
                .border_color(Hsla {
                    h: (336 / 360) as f32,
                    s: 0.75,
                    l: 0.5,
                    a: 1.0,
                })
                .text_color(Hsla {
                    h: (336 / 360) as f32,
                    s: 0.75,
                    l: 0.5,
                    a: 1.0,
                })
                .text_center()
                .child(header.into()),
        )
}
