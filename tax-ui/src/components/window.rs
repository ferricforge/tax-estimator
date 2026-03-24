// components

use gpui::{App, Context, IntoElement, ParentElement, Render, Styled, Subscription, Window, div};
use gpui::{AppContext, ClickEvent, Entity, InteractiveElement, px};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::{Root, h_flex, v_flex};
use gpui_component::{StyledExt, WindowExt};
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "linux"))]
use crate::quit;
use crate::app::spawn_calculate_se_tax;
#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
use crate::components::{EstimatedIncomeForm, SeWorksheetForm, make_button};

pub struct AppWindow {
    _window_close_subscription: Subscription,
    status_message: Option<String>,
    form: Entity<EstimatedIncomeForm>,
    worksheet: Entity<SeWorksheetForm>,
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

        let form = cx.new(|form_cx| EstimatedIncomeForm::new(window, form_cx));
        let worksheet = cx.new(|form_cx| SeWorksheetForm::new(window, form_cx));

        info!("Window constructed");
        Self {
            _window_close_subscription: subscription,
            status_message: None,
            form,
            worksheet,
        }
    }

    fn main_toolbar(
        &self,
        app_window: Entity<AppWindow>,
    ) -> impl IntoElement {
        let form_calc = self.form.clone();
        let view = app_window.clone();

        h_flex()
            .id("window-body")
            .p_1()
            .gap_4()
            .items_center()
            .justify_center()
            .child(make_button(
                "calculate-estimates",
                "Calculate SE Tax",
                move |_click_event: &ClickEvent, window: &mut Window, cx: &mut App| {
                    spawn_calculate_se_tax(form_calc.clone(), window, cx);
                },
            ))
            .child(make_button(
                "open-se-worksheet",
                "SE Worksheet",
                move |_ev: &ClickEvent, window: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| {
                        this.open_se_worksheet_dialog(window, cx);
                    });
                },
            ))
    }

    fn main_body(
        &self,
        app_window: Entity<AppWindow>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_5()
            .gap_4()
            .child(self.form.clone())
            .child(self.main_toolbar(app_window))
    }

    fn render_body(
        &self,
        app_window: Entity<AppWindow>,
    ) -> impl IntoElement {
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

        root.child(self.main_body(app_window))
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

    fn open_se_worksheet_dialog(
        &self,
        window: &mut Window,
        cx: &mut App,
    ) {
        let tax_year: Option<i32> = self.form.read(cx).tax_year(cx);

        self.worksheet.update(cx, |ws, _cx| {
            ws.set_tax_year(tax_year);
        });

        let worksheet_for_dialog = self.worksheet.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .overlay_closable(false)
                .w(px(520.0))
                .margin_top(px(-20.0))
                .title("SE Tax Worksheet")
                .child(worksheet_for_dialog.clone())
                .button_props(DialogButtonProps::default().cancel_text("Close"))
                .footer(|_ok, cancel, window, cx| vec![cancel(window, cx)])
        });
    }
}

impl Render for AppWindow {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app_window: Entity<AppWindow> = cx.entity();
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child(self.render_body(app_window))
            .child(self.render_status_bar())
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
