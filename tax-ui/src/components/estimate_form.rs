use gpui::{
    App, AppContext, ClickEvent, Context, Div, Entity, IntoElement, ParentElement, Render,
    RenderOnce, SharedString, Styled, TextAlign, Window, div, px,
};
use gpui_component::{
    IndexPath, h_flex,
    input::{Input, InputState, MaskPattern},
    select::{Select, SelectState},
    v_flex,
};
use tax_core::FilingStatusCode;

use crate::{
    components::make_button,
    models::EstimatedIncomeModel,
    utils::{parse_decimal, parse_optional_decimal},
};

#[derive(Clone, Debug)]
pub struct EstimatedIncomeForm {
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

        let filing_status = cx.new(|cx| SelectState::new(statuses, initial_index, window, cx));

        let expected_agi =
            make_input_state_with_decimal_mask("Expected adjusted gross income", window, cx);
        let expected_deduction = make_input_state_with_decimal_mask(
            "Expected standard or itemized deduction",
            window,
            cx,
        );
        let expected_qbi_deduction =
            make_input_state_with_decimal_mask("Expected QBI deduction", window, cx);
        let expected_amt =
            make_input_state_with_decimal_mask("Expected alternative minimum tax", window, cx);
        let expected_credits =
            make_input_state_with_decimal_mask("Expected tax credits", window, cx);
        let expected_other_taxes =
            make_input_state_with_decimal_mask("Expected other taxes", window, cx);
        let expected_withholding =
            make_input_state_with_decimal_mask("Expected income tax withheld", window, cx);
        let prior_year_tax =
            make_input_state_with_decimal_mask("Prior year tax liability", window, cx);

        let se_income = make_input_state_with_decimal_mask("Self-employment income", window, cx);
        let expected_crp_payments =
            make_input_state_with_decimal_mask("Expected CRP payments", window, cx);
        let expected_wages = make_input_state_with_decimal_mask("Expected wages", window, cx);

        Self {
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
    pub fn to_model(
        &self,
        cx: &App,
    ) -> Result<EstimatedIncomeModel, anyhow::Error> {
        let filing_status_id: FilingStatusCode = self
            .filing_status
            .read(cx)
            .selected_value()
            .ok_or_else(|| anyhow::anyhow!("No filing status selected"))?
            .as_ref()
            .try_into()?;

        Ok(EstimatedIncomeModel {
            filing_status_id,
            expected_agi: parse_decimal(self.expected_agi.read(cx).value().as_str())?,
            expected_deduction: parse_decimal(self.expected_deduction.read(cx).value().as_str())?,
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
        })
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
                            .child(make_select_row(
                                "Filing Status:",
                                Select::new(&self.filing_status).w_full().render(window, cx),
                            ))
                            .child(make_input_row(&self.se_income, "SE income: $"))
                            .child(make_input_row(
                                &self.expected_crp_payments,
                                "CRP payments: $",
                            ))
                            .child(make_input_row(&self.expected_wages, "Wages: $"))
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
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .size_full()
                            .child("This is a child on the right"),
                    ),
            )
    }
}

fn make_input_state_with_decimal_mask(
    label: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<EstimatedIncomeForm>,
) -> Entity<InputState> {
    let pattern: MaskPattern = MaskPattern::Number {
        separator: Some(','),
        fraction: Some(4),
    };

    cx.new(|closure_cx| {
        InputState::new(window, closure_cx)
            .mask_pattern(pattern)
            .placeholder(label.into())
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
