#[allow(unused_imports)]
use anyhow::{Context as AnyContext, Result};
use gpui::{
    App, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled,
    Window,
};
use gpui_component::{h_flex, input::InputState, v_flex};
use rust_decimal::Decimal;
use tax_core::calculations::SeWorksheetResult;

use crate::{
    app::se_tax_estimate,
    components::{make_button, make_decimal_input, make_display_row, make_input_row_fixed},
    models::SeWorksheetModel,
    utils::parse_optional_decimal,
};

pub struct SeWorksheetForm {
    /// Line 1a: Expected SE income (Form 1040-ES).
    se_income: Entity<InputState>,
    /// Line 1b: Expected CRP payments.
    crp_payments: Entity<InputState>,
    /// Line 6: Expected wages (SS or tier 1 RRTA).
    expected_wages: Entity<InputState>,

    /// Full worksheet model (lines 1a–11).
    model: SeWorksheetModel,
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
            model: SeWorksheetModel::default(),
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

    pub fn total_se_tax(&self) -> Option<Decimal> {
        self.model.line_10_total_se_tax
    }

    pub fn deductible_se_tax(&self) -> Option<Decimal> {
        self.model.line_11_deductible_se_tax
    }

    /// Copies parsed inputs into lines 1a, 1b, 6, and line 2 (1a − 1b). Does not run full SE formulas.
    pub fn calculate_se(
        &mut self,
        cx: &mut Context<'_, SeWorksheetForm>,
    ) -> Result<()> {
        let se_income = self.se_income.read(cx).value();
        let crp_s = self.crp_payments.read(cx).value();
        let wages_s = self.expected_wages.read(cx).value();
    
        let income: Decimal = parse_optional_decimal(se_income.as_str()).unwrap_or(Decimal::ZERO);
        let crp: Decimal = parse_optional_decimal(crp_s.as_str()).unwrap_or(Decimal::ZERO);
    
        self.model.line_1a_expected_se_income = parse_optional_decimal(se_income.as_str());
        self.model.line_1b_expected_crp_payments = parse_optional_decimal(crp_s.as_str());
        self.model.line_6_expected_wages = parse_optional_decimal(wages_s.as_str());
        self.model.line_2_subtract_1b_from_1a = Some(income - crp);
    
        let worksheet = cx.entity().clone();
        call_calculator(worksheet, cx);
        Ok(())
    }

    pub fn clear(
        &mut self,
        window: &mut Window,
        app_cx: &mut App,
    ) {
        self.model = SeWorksheetModel::default();

        let value = SharedString::new("");
        self.se_income.update(
            app_cx,
            |state: &mut InputState, is_cx: &mut Context<'_, InputState>| {
                state.set_value(value.clone(), window, is_cx);
            },
        );

        self.expected_wages.update(
            app_cx,
            |state: &mut InputState, is_cx: &mut Context<'_, InputState>| {
                state.set_value(value.clone(), window, is_cx);
            },
        );
        self.crp_payments.update(
            app_cx,
            |state: &mut InputState, is_cx: &mut Context<'_, InputState>| {
                state.set_value(value, window, is_cx);
            },
        );
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
                "1a. Expected SE income: $",
            ))
            .child(make_input_row_fixed(
                &self.crp_payments,
                "1b. Expected CRP payments: $",
            ))
            .child(make_display_row(
                "2. Subtract line 1b from line 1a:",
                self.model.line_2_subtract_1b_from_1a,
            ))
            .child(make_display_row(
                "3. Multiply line 2 by 92.35% (0.9235):",
                self.model.line_3_net_earnings,
            ))
            .child(make_display_row(
                "4. Multiply line 3 by 2.9% (0.029):",
                self.model.line_4_medicare_tax,
            ))
            .child(make_display_row(
                "5. Social security tax maximum income:",
                self.model.line_5_ss_maximum_income,
            ))
            .child(make_input_row_fixed(
                &self.expected_wages,
                "6. Expected wages (SS / tier 1 RRTA 6.2%): $",
            ))
            .child(make_display_row(
                "7. Subtract line 6 from line 5:",
                self.model.line_7_remaining_ss_base,
            ))
            .child(make_display_row(
                "8. Smaller of line 3 or line 7:",
                self.model.line_8_ss_taxable_earnings,
            ))
            .child(make_display_row(
                "9. Multiply line 8 by 12.4% (0.124):",
                self.model.line_9_social_security_tax,
            ))
            .child(make_display_row(
                "10. Add lines 4 and 9:",
                self.model.line_10_total_se_tax,
            ))
            .child(make_display_row(
                "11. Multiply line 10 by 50% (0.50):",
                self.model.line_11_deductible_se_tax,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .mt_4()
                    .child(make_button("calculate_se_tax", "Calculate", {
                        let this = this.clone();
                        move |_ev, _window, cx| {
                            this.update(cx, |form: &mut SeWorksheetForm, cx: &mut Context<'_, SeWorksheetForm>| {
                                let _ = form.calculate_se(cx);
                                cx.notify();
                            });
                        }
                    }))
                    .child(make_button(
                        "calculate_se_clear",
                        "Clear",
                        move |_ev: &ClickEvent, window: &mut Window, app_cx: &mut App| {
                            this.update(app_cx, |form, cx| {
                                form.clear(window, cx);
                            });
                        },
                    )),
            )
    }
}

fn call_calculator(
    worksheet: Entity<SeWorksheetForm>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let model = cx
            .update(|cx| worksheet.read(cx).model.clone())
            .unwrap();

        match make_se_estimate(model).await {
            Ok(result) => {
                cx.update(|cx| {
                    worksheet.update(cx, |form, cx| {
                        form.model.from_worksheet_result(&result);
                        cx.notify();
                    });
                })
                .ok();
            }
            Err(e) => {
                tracing::warn!(%e, "Calculate SE Tax failed");
            }
        }
    })
    .detach();
}

async fn make_se_estimate(model: SeWorksheetModel) -> Result<SeWorksheetResult> {
    let se_income = model.line_1a_expected_se_income.unwrap_or_default();
    let crp_payments = model.line_1b_expected_crp_payments.unwrap_or_default();
    let wages = model.line_6_expected_wages.unwrap_or_default();
    let tax_year = model.tax_year.unwrap_or_default();
    se_tax_estimate(
        se_income,
        crp_payments,
        wages,
        tax_year,
        "taxes.db",
        "sqlite",
    )
    .await
}
