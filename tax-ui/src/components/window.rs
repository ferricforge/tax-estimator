// components

use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Subscription, Window, div};
use gpui::{AppContext, Entity, px};
use gpui_component::StyledExt;
use gpui_component::{Root, v_flex};
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
use crate::components::{EstimatedIncomeForm, SeWorksheetForm};
#[cfg(not(target_os = "linux"))]
use crate::quit;

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
