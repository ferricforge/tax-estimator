use gpui::{
    App, AppContext, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Window,
};
use gpui_component::{
    IndexPath, h_flex,
    select::{Select, SelectState},
    v_flex,
};
use tax_core::TaxEstimate;

use crate::components::make_button;

/// Dropdown selector over a list of previously saved [`TaxEstimate`] records.
///
/// The **Select** button logs the chosen estimate at `INFO` level. In a
/// future step this will hydrate the main 1040-ES form instead.
pub struct EstimateSelector {
    estimates: Vec<TaxEstimate>,
    labels: Vec<SharedString>,
    select: Entity<SelectState<Vec<SharedString>>>,
}

impl EstimateSelector {
    pub fn new(
        estimates: Vec<TaxEstimate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let labels: Vec<SharedString> = estimates
            .iter()
            .map(|e| {
                SharedString::from(format!(
                    "#{} — {} {} (updated {})",
                    e.id,
                    e.input.tax_year,
                    e.input.filing_status.as_str(),
                    e.updated_at.format("%Y-%m-%d %H:%M"),
                ))
            })
            .collect();

        let initial = if labels.is_empty() {
            None
        } else {
            Some(IndexPath::default().row(0))
        };

        let select = cx.new(|cx| SelectState::new(labels.clone(), initial, window, cx));

        Self {
            estimates,
            labels,
            select,
        }
    }

    /// Returns the currently selected estimate, if any.
    pub fn selected_estimate(
        &self,
        cx: &App,
    ) -> Option<&TaxEstimate> {
        let selected_label = self.select.read(cx).selected_value()?;
        let idx = self
            .labels
            .iter()
            .position(|label| label.as_ref() == selected_label.as_ref())?;
        self.estimates.get(idx)
    }
}

impl Render for EstimateSelector {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();

        v_flex()
            .gap_2()
            .p_4()
            .child(Select::new(&self.select).w_full().render(window, cx))
            .child(h_flex().justify_end().mt_4().child(make_button(
                "select-estimate",
                "Select",
                true,
                move |_ev: &ClickEvent, _window: &mut Window, cx: &mut App| {
                    this.update(cx, |selector, sel_cx| {
                        if let Some(estimate) = selector.selected_estimate(sel_cx) {
                            tracing::info!("Selected estimate: {}", estimate);
                        } else {
                            tracing::info!("No estimate selected");
                        }
                    });
                },
            )))
    }
}
