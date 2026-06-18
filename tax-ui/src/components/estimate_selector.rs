use std::rc::Rc;

use gpui::{
    App, AppContext, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, Styled, Subscription, Window,
};
use gpui_component::{
    IndexPath, WindowExt, h_flex,
    select::{Select, SelectState},
    v_flex,
};
use tax_core::TaxEstimate;

use crate::components::make_button;

/// Dropdown selector over a list of previously saved [`TaxEstimate`] records.
///
/// Renders a dropdown with **Select** and **Cancel** buttons beneath it. The
/// dropdown's first row is blank, representing "no selection". **Select** is
/// disabled while the blank row is chosen; once a real estimate is selected it
/// becomes enabled, fires the provided callback with that estimate, and
/// dismisses the dialog. **Cancel** dismisses the dialog without action.
pub struct EstimateSelector {
    estimates: Vec<TaxEstimate>,
    labels: Vec<SharedString>,
    select: Entity<SelectState<Vec<SharedString>>>,
    on_select: Rc<dyn Fn(&TaxEstimate, &mut Window, &mut App)>,
    _select_subscription: Subscription,
}

impl EstimateSelector {
    /// Creates a new selector from saved estimates.
    ///
    /// `on_select` is invoked with the chosen estimate when the user clicks
    /// **Select**.
    pub fn new(
        estimates: Vec<TaxEstimate>,
        on_select: Rc<dyn Fn(&TaxEstimate, &mut Window, &mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut labels: Vec<SharedString> = vec![SharedString::from("")];
        labels.extend(estimates.iter().map(|e| {
            SharedString::from(format!(
                "#{} — {} {} (updated {})",
                e.id,
                e.input.tax_year,
                e.input.filing_status.as_str(),
                e.updated_at.format("%Y-%m-%d %H:%M"),
            ))
        }));

        let initial = Some(IndexPath::default().row(0));

        let select = cx.new(|cx| SelectState::new(labels.clone(), initial, window, cx));

        let subscription = cx.observe(&select, |_this, _select, cx| {
            cx.notify();
        });

        Self {
            estimates,
            labels,
            select,
            on_select,
            _select_subscription: subscription,
        }
    }

    /// Returns the currently selected estimate, if any.
    ///
    /// The blank first row yields `None`.
    pub fn selected_estimate(
        &self,
        cx: &App,
    ) -> Option<&TaxEstimate> {
        let selected_label = self.select.read(cx).selected_value()?;
        let idx = self
            .labels
            .iter()
            .position(|label| label.as_ref() == selected_label.as_ref())?;
        if idx == 0 {
            return None;
        }
        self.estimates.get(idx - 1)
    }
}

impl Render for EstimateSelector {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let this = cx.entity().clone();
        let on_select = self.on_select.clone();
        let can_select = self.selected_estimate(cx).is_some();

        v_flex()
            .gap_2()
            .p_4()
            .child(Select::new(&self.select).w_full().render(window, cx))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .mt_4()
                    .child(make_button(
                        "select-estimate",
                        "Select",
                        can_select,
                        move |_ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                            let selected = this.read(cx).selected_estimate(cx).cloned();
                            if let Some(ref estimate) = selected {
                                on_select(estimate, window, cx);
                            } else {
                                tracing::info!("No estimate selected");
                            }
                            window.close_dialog(cx);
                        },
                    ))
                    .child(make_button(
                        "cancel-estimate",
                        "Cancel",
                        true,
                        |_ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                            window.close_dialog(cx);
                        },
                    )),
            )
    }
}
