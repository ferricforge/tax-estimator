//! The application's root view.
//!
//! `AppWindow` owns the estimate form and the status line, wires the File
//! menu actions to their handlers, and renders the window. The handlers
//! themselves live in sibling files: `load_estimate` and `connection_actions`.

mod connection_actions;
mod load_estimate;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement, Render, Styled, Subscription, Window, div, px,
};
use gpui_component::{Root, StyledExt, v_flex};
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
use crate::components::{
    EstimatedIncomeForm, LoadEstimate, NewConnection, OpenConnection, OpenRecentConnection,
    SaveConnection, SaveConnectionAs, SeWorksheetForm, save_window_bounds,
};
use crate::config::MAIN_WINDOW;
#[cfg(not(target_os = "linux"))]
use crate::quit;

gpui::actions!(tax_estimator, [ReloadConnection]);

pub struct AppWindow {
    _window_close_subscription: Subscription,
    focus_handle: FocusHandle,
    status_message: Option<String>,
    form: Entity<EstimatedIncomeForm>,
}

impl AppWindow {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Closing the last window quits the application. Today the main
        // window is the only window, so this always fires when it closes.
        // It will also run, harmlessly, when a future window (for example,
        // preferences) outlives the main window and is then closed itself.
        //
        // The Linux branch is intentionally excluded here; the reason has
        // not yet been confirmed against gpui's Linux platform code. See the
        // preferences editor design notes for how to investigate this.
        let subscription = cx.on_window_closed(|app_cx: &mut App| {
            info!("Window closed callback");
            #[cfg(not(target_os = "linux"))]
            if app_cx.windows().is_empty() {
                quit(&Quit, app_cx);
            }
        });

        // Saves the window's position and size before it closes, so the
        // next launch can restore them.
        window.on_window_should_close(cx, |window, app_cx| {
            save_window_bounds(MAIN_WINDOW, window, app_cx);
            true
        });

        // Actions are dispatched along the focus path, so the root element
        // must be focused (or an ancestor of the focused element) for its
        // `on_action` handlers to run. Focus it up front so menu actions work
        // before the user has clicked into any field.
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let worksheet = cx.new(|form_cx| SeWorksheetForm::new(window, form_cx));
        let form = cx.new(|form_cx| EstimatedIncomeForm::new(worksheet.clone(), window, form_cx));

        info!("Window constructed");
        Self {
            _window_close_subscription: subscription,
            focus_handle,
            status_message: None,
            form,
        }
    }

    fn set_status(
        &mut self,
        message: impl Into<String>,
    ) {
        self.status_message = Some(message.into());
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
            .key_context("AppWindow")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &LoadEstimate, window, cx| {
                this.handle_load_estimate(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewConnection, window, cx| {
                this.handle_new_connection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenConnection, window, cx| {
                this.handle_open_connection(window, cx);
            }))
            .on_action(
                cx.listener(|this, action: &OpenRecentConnection, window, cx| {
                    this.handle_open_recent_connection(&action.connection, window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &SaveConnection, window, cx| {
                this.handle_save_connection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveConnectionAs, window, cx| {
                this.handle_save_connection_as(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReloadConnection, window, cx| {
                this.handle_reload_connection(window, cx);
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

impl Focusable for AppWindow {
    fn focus_handle(
        &self,
        _cx: &App,
    ) -> FocusHandle {
        self.focus_handle.clone()
    }
}
