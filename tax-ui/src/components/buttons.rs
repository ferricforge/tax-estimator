use gpui::{App, ClickEvent, SharedString, Styled, Window, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Sizable};

/// Creates a primary-styled button with a custom click handler.
pub fn make_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id.into())
        .primary()
        .large()
        .w(px(140.))
        .label(label.into())
        .disabled(!enabled)
        .on_click(on_click)
}
