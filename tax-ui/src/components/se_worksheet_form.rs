use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window,
};
use gpui_component::{h_flex, input::InputState, v_flex};
use rust_decimal::Decimal;

use crate::{
    components::{make_button, make_decimal_input, make_display_row, make_input_row_fixed},
    utils::parse_optional_decimal,
};

pub struct SeWorksheetForm {
    /// Line 1a: Net profit from self-employment
    se_income: Entity<InputState>,
    /// Line 1b: CRP payments included on Schedule SE
    crp_payments: Entity<InputState>,
    /// Line 4a: Wages subject to social security tax
    expected_wages: Entity<InputState>,

    /// Line 2: Net SE earnings (line 1a × 92.35%)
    line_2_net_se_earnings: Option<Decimal>,
    /// Line 3: Social security wage base for the year
    line_3_ss_wage_base: Option<Decimal>,
    /// Line 5: Total wages subject to SS
    line_5_total_wages: Option<Decimal>,
    /// Line 6: SS wage base minus wages (line 3 - line 5)
    line_6_remaining_base: Option<Decimal>,
    /// Line 7: Amount subject to SS tax (lesser of line 2 or line 6)
    line_7_ss_taxable: Option<Decimal>,
    /// Line 8: Social security tax (line 7 × 12.4%)
    line_8_ss_tax: Option<Decimal>,
    /// Line 9: Medicare tax (line 2 × 2.9%)
    line_9_medicare_tax: Option<Decimal>,
    /// Line 10: Total SE tax (line 8 + line 9)
    line_10_total_se_tax: Option<Decimal>,
    /// Line 11: Deductible SE tax (line 10 × 50%)
    line_11_deductible: Option<Decimal>,
}

impl SeWorksheetForm {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            se_income: make_decimal_input("Net SE income", 2, window, cx),
            crp_payments: make_decimal_input("CRP payments", 2, window, cx),
            expected_wages: make_decimal_input("Expected wages", 2, window, cx),
            line_2_net_se_earnings: None,
            line_3_ss_wage_base: None,
            line_5_total_wages: None,
            line_6_remaining_base: None,
            line_7_ss_taxable: None,
            line_8_ss_tax: None,
            line_9_medicare_tax: None,
            line_10_total_se_tax: None,
            line_11_deductible: None,
        }
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
        line_2: Option<Decimal>,
        line_3: Option<Decimal>,
        line_5: Option<Decimal>,
        line_6: Option<Decimal>,
        line_7: Option<Decimal>,
        line_8: Option<Decimal>,
        line_9: Option<Decimal>,
        line_10: Option<Decimal>,
        line_11: Option<Decimal>,
    ) {
        self.line_2_net_se_earnings = line_2;
        self.line_3_ss_wage_base = line_3;
        self.line_5_total_wages = line_5;
        self.line_6_remaining_base = line_6;
        self.line_7_ss_taxable = line_7;
        self.line_8_ss_tax = line_8;
        self.line_9_medicare_tax = line_9;
        self.line_10_total_se_tax = line_10;
        self.line_11_deductible = line_11;
    }

    pub fn total_se_tax(&self) -> Option<Decimal> {
        self.line_10_total_se_tax
    }

    pub fn deductible_se_tax(&self) -> Option<Decimal> {
        self.line_11_deductible
    }
}

impl Render for SeWorksheetForm {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();

        v_flex()
            .gap_2()
            .p_4()
            .child(make_input_row_fixed(
                &self.se_income,
                "1a. Net SE income: $",
            ))
            .child(make_input_row_fixed(
                &self.crp_payments,
                "1b. CRP payments: $",
            ))
            .child(make_display_row(
                "2. Net SE earnings (1a × 92.35%):",
                self.line_2_net_se_earnings,
            ))
            .child(make_display_row(
                "3. Social security wage base:",
                self.line_3_ss_wage_base,
            ))
            .child(make_input_row_fixed(
                &self.expected_wages,
                "4a. Wages subject to SS: $",
            ))
            .child(make_display_row(
                "5. Total wages (line 4a):",
                self.line_5_total_wages,
            ))
            .child(make_display_row(
                "6. Line 3 minus line 5:",
                self.line_6_remaining_base,
            ))
            .child(make_display_row(
                "7. Smaller of line 2 or 6:",
                self.line_7_ss_taxable,
            ))
            .child(make_display_row(
                "8. SS tax (line 7 × 12.4%):",
                self.line_8_ss_tax,
            ))
            .child(make_display_row(
                "9. Medicare (line 2 × 2.9%):",
                self.line_9_medicare_tax,
            ))
            .child(make_display_row(
                "10. Total SE tax:",
                self.line_10_total_se_tax,
            ))
            .child(make_display_row(
                "11. Deductible (line 10 × 50%):",
                self.line_11_deductible,
            ))
            .child(h_flex().gap_2().justify_end().mt_4().child(make_button(
                "se_calculate",
                "Calculate",
                move |_ev, _window, cx| {
                    this.update(cx, |form, cx| {
                        // Smoke test only: line 2 = 1a - 1b
                        let income =
                            parse_optional_decimal(form.se_income.read(cx).value().as_str())
                                .unwrap_or(Decimal::ZERO);
                        let crp =
                            parse_optional_decimal(form.crp_payments.read(cx).value().as_str())
                                .unwrap_or(Decimal::ZERO);
                        form.line_2_net_se_earnings = Some(income - crp);
                        cx.notify();
                    });
                },
            )))
    }
}
