// This form is designed to display the results of tax calculations
//
// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// pub struct TaxEstimateComputed {
//     pub se_tax: Decimal,
//     pub total_tax: Decimal,
//     pub required_payment: Decimal,
// }

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::v_flex;
use rust_decimal::Decimal;
use tax_core::TaxEstimateComputed;
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

    /// Fills display fields from a previously persisted [`TaxEstimateComputed`].
    pub fn set_from_computed(
        &mut self,
        computed: &TaxEstimateComputed,
    ) {
        self.calculated_se_tax = Some(computed.se_tax);
        self.calculated_total_tax = Some(computed.total_tax);
        self.calculated_payment = Some(computed.required_payment);
    }

    /// Resets the form so no results are displayed.
    pub fn clear(&mut self) {
        self.calculated_se_tax = None;
        self.calculated_total_tax = None;
        self.calculated_payment = None;
    }
}

impl Render for ResultForm {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_4()
            .child(make_header_row("Calculated Results"))
            .child(make_display_row(SE_LABEL, self.calculated_se_tax))
            .child(make_display_row(TOTAL_TAX_LABEL, self.calculated_total_tax))
            .child(make_display_row(PAYMENTS_LABEL, self.calculated_payment))
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn set_from_computed_populates_all_fields() {
        let computed = TaxEstimateComputed {
            se_tax: dec!(7500.00),
            total_tax: dec!(25000.00),
            required_payment: dec!(4000.00),
        };
        let mut form = ResultForm::default();
        form.set_from_computed(&computed);

        assert_eq!(form.calculated_se_tax, Some(dec!(7500.00)));
        assert_eq!(form.calculated_total_tax, Some(dec!(25000.00)));
        assert_eq!(form.calculated_payment, Some(dec!(4000.00)));
        assert_eq!(form.has_results(), true);
    }

    #[test]
    fn clear_resets_all_fields() {
        let mut form = ResultForm {
            calculated_se_tax: Some(dec!(1.00)),
            calculated_total_tax: Some(dec!(2.00)),
            calculated_payment: Some(dec!(3.00)),
        };
        form.clear();

        assert_eq!(form.calculated_se_tax, None);
        assert_eq!(form.calculated_total_tax, None);
        assert_eq!(form.calculated_payment, None);
        assert_eq!(form.has_results(), false);
    }
}
