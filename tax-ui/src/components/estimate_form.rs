use gpui::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render, RenderOnce, SharedString,
    Styled, Window, div,
};
use gpui_component::{
    IndexPath, h_flex,
    input::{InputEvent, InputState},
    select::{Select, SelectState},
    v_flex,
};
use regex::Regex;

use crate::{
    components::{
        make_decimal_input, make_header_row, make_input_row, make_integer_input, make_labeled_row,
        make_select_row,
    },
    models::EstimatedIncomeModel,
    repository::ActiveTaxYear,
    utils::{parse_decimal, parse_optional_decimal},
};

#[derive(Clone, Debug)]
pub struct EstimatedIncomeForm {
    tax_year: Entity<InputState>,
    filing_status: Entity<SelectState<Vec<SharedString>>>,

    // SE Worksheet inputs
    se_income: Entity<InputState>,
    expected_crp_payments: Entity<InputState>,
    expected_wages: Entity<InputState>,

    // 1040-ES Worksheet inputs
    expected_agi: Entity<InputState>,
    expected_deduction: Entity<InputState>,
    expected_qbi_deduction: Entity<InputState>,
    expected_amt: Entity<InputState>,
    expected_credits: Entity<InputState>,
    expected_other_taxes: Entity<InputState>,
    expected_withholding: Entity<InputState>,
    prior_year_tax: Entity<InputState>,
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

        let tax_year = make_integer_input("Tax Year", window, cx);
        tax_year.update(cx, |input_state, cx| {
            input_state.set_pattern(Regex::new(r"^\d{0,4}$").unwrap(), window, cx);
        });

        // React to edits: once a full 4-digit year is entered, fetch its config.
        cx.subscribe(&tax_year, |_this, input, event, cx| {
            if let InputEvent::Change = event {
                let raw = input.read(cx).value();
                if let Ok(year) = raw.trim().parse::<i32>() {
                    if (1900..=2200).contains(&year) {
                        ActiveTaxYear::load(year, cx);
                    }
                }
            }
        })
        .detach();

        let filing_status = cx.new(|cx| SelectState::new(statuses, initial_index, window, cx));

        Self {
            tax_year,
            filing_status,
            se_income: make_decimal_input("Self-emp income", 2, window, cx),
            expected_crp_payments: make_decimal_input("Exp CRP payments", 2, window, cx),
            expected_wages: make_decimal_input("Exp wages", 2, window, cx),
            expected_agi: make_decimal_input("Exp AGI", 2, window, cx),
            expected_deduction: make_decimal_input("Exp deduction", 2, window, cx),
            expected_qbi_deduction: make_decimal_input("Exp QBI deduction", 2, window, cx),
            expected_amt: make_decimal_input("Exp AMT", 2, window, cx),
            expected_credits: make_decimal_input("Exp tax credits", 2, window, cx),
            expected_other_taxes: make_decimal_input("Exp other taxes", 2, window, cx),
            expected_withholding: make_decimal_input("Exp inc tax withheld", 2, window, cx),
            prior_year_tax: make_decimal_input("Prior year tax liability", 2, window, cx),
        }
    }

    /// Collects the current form values into an [`EstimatedIncomeModel`].
    ///
    /// Runs parse/required-field checks then business-rule validation. Returns
    /// all errors so the user can see every problem at once.
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
            se_income: parse_optional_decimal(self.se_income.read(cx).value().as_str()),
            expected_crp_payments: parse_optional_decimal(
                self.expected_crp_payments.read(cx).value().as_str(),
            ),
            expected_wages: parse_optional_decimal(self.expected_wages.read(cx).value().as_str()),
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
        };

        model.validate_for_submit()?;
        Ok(model)
    }

    /// Returns the raw tax year value, parsed if valid.
    pub fn tax_year(
        &self,
        cx: &App,
    ) -> Option<i32> {
        let value = self.tax_year.read(cx).value();
        value.trim().parse::<i32>().ok()
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
                            .child(make_input_row(&self.tax_year, "Tax Year"))
                            .child(make_select_row(
                                "Filing Status:",
                                Select::new(&self.filing_status).w_full().render(window, cx),
                            )),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .size_full()
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
                            .child(make_input_row(&self.prior_year_tax, "Prior year tax: $")),
                    ),
            )
    }
}
