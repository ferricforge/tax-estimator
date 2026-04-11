
// This form is deisgned to display the results of tax calculations
//
// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub struct TaxEstimateComputed {
//     pub se_tax: Decimal,
//     pub total_tax: Decimal,
//     pub required_payment: Decimal,
// }

use gpui::{Context, Div, IntoElement, ParentElement, Render, Styled};
use gpui_component::v_flex;
use rust_decimal::Decimal;

use crate::components::make_header_row;

const SE_LABEL: &str = "Self-Employment Tax";
const TOTAL_TAX_LABEL: &str = "Total Tax Due";
const PAYMENTS_LABEL: &str = "Quarterly Payment";

#[derive(Clone, Debug)]
pub struct ResultForm {
    calculated_se_tax: Option<Decimal>,
    calculated_total_tax: Option<Decimal>,
    calculated_payment: Option<Decimal>,
}

impl ResultForm {
    fn render_se_row(&self, cx: &mut Context<Self>) -> Div {
        todo!()
    }
    fn render_total_row(&self, cx: &mut Context<Self>) -> Div {
        todo!()
    }
    fn render_payments_row(&self, cx: &mut Context<Self>) -> Div {
        todo!()
    }
}

impl Render for ResultForm {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .child(make_header_row("Calculated Results"))
            .child(self.render_se_row(cx))
            .child(self.render_total_row(cx))
            .child(self.render_payments_row(cx))
    }
}