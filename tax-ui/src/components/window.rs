// components

use std::rc::Rc;

use gpui::{
    App, AppContext, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, px,
};
use gpui_component::{Root, StyledExt, WindowExt, v_flex};
use tax_core::TaxEstimate;
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
use crate::components::{
    EstimateSelector, EstimatedIncomeForm, LoadEstimate, SeWorksheetForm, show_err,
};
#[cfg(not(target_os = "linux"))]
use crate::quit;
use crate::repository::TaxRepo;

pub struct AppWindow {
    _window_close_subscription: Subscription,
    status_message: Option<String>,
    form: Entity<EstimatedIncomeForm>,
}

impl AppWindow {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.on_window_closed(|_cx: &mut App| {
            info!("Window closed callback");
            #[cfg(not(target_os = "linux"))]
            quit(&Quit, _cx);
        });

        let worksheet = cx.new(|form_cx| SeWorksheetForm::new(window, form_cx));
        let form = cx.new(|form_cx| EstimatedIncomeForm::new(worksheet.clone(), window, form_cx));

        info!("Window constructed");
        Self {
            _window_close_subscription: subscription,
            status_message: None,
            form,
        }
    }

    /// Fetches saved estimates and opens a selector dialog.
    fn handle_load_estimate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = TaxRepo::try_get(cx) else {
            tracing::warn!("TaxRepo not initialised; cannot load estimates");
            return;
        };

        tracing::info!("Loading saved estimates");
        let window_handle = window.window_handle();

        cx.spawn(
            async move |this, async_cx| match repo.list_estimates(None).await {
                Ok(estimates) if estimates.is_empty() => {
                    tracing::info!("No saved estimates found");
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
                            let on_select: Rc<dyn Fn(&TaxEstimate, &mut Window, &mut App)> =
                                Rc::new(move |estimate, window, cx| {
                                    tracing::info!("Selected estimate: {}", estimate);
                                    form.update(cx, |form, form_cx| {
                                        form.populate_from_estimate(&estimate, window, form_cx);
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
                    tracing::error!(error = %e, "Failed to load estimates");
                    show_err(window_handle, async_cx, e.into());
                }
            },
        )
        .detach();
    }

    fn main_body(&self) -> impl IntoElement {
        v_flex().size_full().p_5().gap_4().child(self.form.clone())
    }

    fn render_body(&self) -> impl IntoElement {
        let root = {
            let base = v_flex().size_full().gap_0();
            #[cfg(not(target_os = "macos"))]
            {
                base.child(build_menu_bar())
            }
            #[cfg(target_os = "macos")]
            {
                base
            }
        };

        root.child(self.main_body())
    }

    fn render_status_bar(&self) -> impl IntoElement {
        let status_text = self
            .status_message
            .clone()
            .unwrap_or_else(|| "Ready".to_string());

        div()
            .w_full()
            .px_3()
            .py_2()
            .border_t_1()
            .child(div().text_size(px(11.0)).child(status_text))
    }
}

impl Render for AppWindow {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("app-window")
            .on_action(cx.listener(|this, _: &LoadEstimate, window, cx| {
                this.handle_load_estimate(window, cx);
            }))
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .child(self.render_body())
            .child(self.render_status_bar())
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
