//! Confirmation shown before a worksheet's Clear button discards its data.
//!
//! Clearing changes only the form. The data is removed from the database the
//! next time the estimate is saved, so the dialog says so. The "Don't show
//! this again" choice is saved in the `[dialogs]` section of the
//! configuration, and can be undone from Preferences.

use std::rc::Rc;

use gpui::{
    App, AppContext, Context, IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::{WindowExt, h_flex, v_flex};

use crate::components::make_button;
use crate::config::AppConfig;

/// Clears a worksheet after the user confirms, or at once when they have
/// turned the confirmation off for `key`.
///
/// `key` identifies the worksheet, `message` explains what clearing does, and
/// `on_confirm` performs the clear.
pub fn confirm_clear(
    key: &'static str,
    message: &'static str,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    if AppConfig::get(cx).dialogs.is_suppressed(key) {
        on_confirm(window, cx);
        return;
    }

    let view = cx.new(|_| ClearConfirmView {
        key,
        message,
        skip_future: false,
        on_confirm: Rc::new(on_confirm),
    });

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .overlay_closable(false)
            .w(px(420.0))
            .title("Clear this form?")
            .child(view.clone())
    });
}

/// Records that `key` should no longer be shown, and saves the choice.
fn suppress(
    key: &'static str,
    cx: &mut App,
) {
    AppConfig::update(cx, |config| config.dialogs.suppress(key));

    if let Err(error) = AppConfig::save(cx) {
        tracing::error!(%error, "failed to save the hidden confirmation");
    }
}

/// Runs the worksheet clear after the user confirms.
type OnConfirm = Rc<dyn Fn(&mut Window, &mut App)>;

/// The body of the confirmation dialog. It holds the checkbox state and
/// draws its own buttons.
struct ClearConfirmView {
    key: &'static str,
    message: &'static str,
    skip_future: bool,
    on_confirm: OnConfirm,
}

impl Render for ClearConfirmView {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let key = self.key;
        let skip_future = self.skip_future;
        let on_confirm = self.on_confirm.clone();

        v_flex()
            .gap_4()
            .child(div().child(self.message))
            .child(
                Checkbox::new("clear-confirm-skip")
                    .label("Don't show this again")
                    .checked(self.skip_future)
                    .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                        this.skip_future = *checked;
                        cx.notify();
                    })),
            )
            .child(
                h_flex()
                    .gap_2()
                    .justify_end()
                    .child(make_button(
                        "clear-confirm-cancel",
                        "Cancel",
                        true,
                        |_ev, window, cx| {
                            window.close_dialog(cx);
                        },
                    ))
                    .child(make_button(
                        "clear-confirm-ok",
                        "Clear",
                        true,
                        move |_ev, window, cx| {
                            if skip_future {
                                suppress(key, cx);
                            }
                            window.close_dialog(cx);
                            (*on_confirm)(window, cx);
                        },
                    )),
            )
    }
}
