// This form is designed to display the results of tax calculations
//
// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub struct TaxEstimateComputed {
//     pub se_tax: Decimal,
//     pub total_tax: Decimal,
//     pub required_payment: Decimal,
// }

use gpui::{Context, Div, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::v_flex;
use rust_decimal::Decimal;
use tax_core::calculations::EstimatedTaxWorksheetResult;

use crate::components::{make_display_row, make_header_row};

const SE_LABEL: &str = "Self-Employment Tax";
const TOTAL_TAX_LABEL: &str = "Total Tax Due";
const PAYMENTS_LABEL: &str = "Required annual payment";

/// Read-only summary of the last successful estimated-tax calculation.
#[derive(Clone, Debug, Default)]
pub struct ResultForm {
    calculated_se_tax: Option<Decimal>,
    calculated_total_tax: Option<Decimal>,
    calculated_payment: Option<Decimal>,
}

impl ResultForm {
    /// Whether a successful calculation has populated this form.
    pub fn has_results(&self) -> bool {
        self.calculated_total_tax.is_some()
    }

    /// Fills display fields from worksheet output (matches [`TaxEstimateComputed`] / save path).
    pub fn set_from_calculation(
        &mut self,
        se_tax: Decimal,
        result: &EstimatedTaxWorksheetResult,
    ) {
        self.calculated_se_tax = Some(se_tax);
        self.calculated_total_tax = Some(result.total_estimated_tax);
        self.calculated_payment = Some(result.required_annual_payment);
    }

    fn render_se_row(
        &self,
        _cx: &mut Context<Self>,
    ) -> Div {
        make_display_row(SE_LABEL, self.calculated_se_tax)
    }

    fn render_total_row(
        &self,
        _cx: &mut Context<Self>,
    ) -> Div {
        make_display_row(TOTAL_TAX_LABEL, self.calculated_total_tax)
    }

    fn render_payments_row(
        &self,
        _cx: &mut Context<Self>,
    ) -> Div {
        make_display_row(PAYMENTS_LABEL, self.calculated_payment)
    }
}

impl Render for ResultForm {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .child(make_header_row("Calculated Results"))
            .child(self.render_se_row(cx))
            .child(self.render_total_row(cx))
            .child(self.render_payments_row(cx))
    }
}
