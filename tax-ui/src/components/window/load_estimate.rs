//! *Load Estimate* handler for [`AppWindow`].

use std::rc::Rc;

use gpui::{AppContext, Context, ParentElement, Window, px};
use gpui_component::WindowExt;

use super::AppWindow;
use crate::components::estimate_selector::OnSelectEstimate;
use crate::components::{ErrorDialog, EstimateSelector, InfoDialog, show_err};
use crate::repository::TaxRepo;

impl AppWindow {
    /// Fetches saved estimates and opens a selector dialog.
    pub(super) fn handle_load_estimate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = TaxRepo::try_get(cx) else {
            tracing::warn!("TaxRepo not initialised; cannot load estimates");
            ErrorDialog::show(
                "Cannot load estimates",
                &["The database connection is not available.".to_string()],
                window,
                cx,
            );
            return;
        };

        tracing::info!("Loading saved estimates");
        let window_handle = window.window_handle();

        cx.spawn(
            async move |this, async_cx| match repo.list_estimates(None).await {
                Ok(estimates) if estimates.is_empty() => {
                    tracing::info!("No saved estimates found");
                    let _ = window_handle.update(async_cx, |_, window, cx| {
                        InfoDialog::show(
                            "No saved estimates",
                            "There are no saved estimates to load yet. Calculate an estimate \
                             and it will be saved automatically.",
                            window,
                            cx,
                        );
                    });
                }
                Ok(estimates) => {
                    tracing::info!("Found {} saved estimate(s)", estimates.len());
                    for estimate in &estimates {
                        tracing::info!("{}", estimate);
                    }
                    let _ = window_handle.update(async_cx, |_, window, cx| {
                        let _ = this.update(cx, move |app_window, view_cx| {
                            let mut estimates_opt = Some(estimates);
                            let form = app_window.form.clone();
                            let on_select: OnSelectEstimate =
                                Rc::new(move |estimate, window, cx| {
                                    tracing::info!("Selected estimate: {}", estimate);
                                    form.update(cx, |form, form_cx| {
                                        form.populate_from_estimate(estimate, window, form_cx);
                                    });
                                });
                            let selector = view_cx.new(|sel_cx| {
                                EstimateSelector::new(
                                    estimates_opt.take().unwrap(),
                                    on_select,
                                    window,
                                    sel_cx,
                                )
                            });
                            window.open_dialog(view_cx, move |dialog, _w, _cx| {
                                dialog
                                    .title("Load Saved Estimate")
                                    .w(px(500.0))
                                    .child(selector.clone())
                            });
                        });
                    });
                }
                Err(e) => {
                    let e = anyhow::Error::from(e).context("Could not load saved estimates");
                    tracing::error!(error = ?e, "Failed to load estimates");
                    show_err(window_handle, async_cx, "Load failed", &e);
                }
            },
        )
        .detach();
    }
}
