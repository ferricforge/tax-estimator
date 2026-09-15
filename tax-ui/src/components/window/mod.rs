//! The application's root view.
//!
//! `AppWindow` owns the estimate form and the status line, wires the File
//! menu actions to their handlers, and renders the window. The handlers
//! themselves live in sibling files: `load_estimate` and `project_actions`.

mod load_estimate;
mod project_actions;

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
    EstimatedIncomeForm, LoadEstimate, NewProject, OpenProject, SaveProject, SaveProjectAs,
    SeWorksheetForm,
};
#[cfg(not(target_os = "linux"))]
use crate::quit;

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
        let subscription = cx.on_window_closed(|_cx: &mut App| {
            info!("Window closed callback");
            #[cfg(not(target_os = "linux"))]
            quit(&Quit, _cx);
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
            .on_action(cx.listener(|this, _: &NewProject, window, cx| {
                this.handle_new_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenProject, window, cx| {
                this.handle_open_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveProject, window, cx| {
                this.handle_save_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveProjectAs, window, cx| {
                this.handle_save_project_as(window, cx);
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
