pub mod dialogs;
mod estimate_form;
mod file_menu;
pub mod file_picker;
pub mod filters;
mod se_worksheet_form;
mod theme;
pub mod window;

use gpui::{
    App, AppContext, Context, Div, Entity, IntoElement, ParentElement, SharedString, TextAlign,
    Window, div,
};
use gpui::{ClickEvent, Styled};
use gpui::{Pixels, Size, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Disableable, Sizable, h_flex};

pub use dialogs::ErrorDialog;
pub use estimate_form::EstimatedIncomeForm;

pub use file_menu::{
    CloseProject, NewProject, OpenProject, SaveProject, SaveProjectAs, bind_menu_keys,
    build_menu_bar,
};
use gpui_component::input::{Input, InputState, MaskPattern};
use rust_decimal::Decimal;
pub use se_worksheet_form::SeWorksheetForm;
pub use theme::init_theme_colors;
pub use window::AppWindow;

#[derive(Debug, Clone, Copy)]
pub struct WindowPreferences {
    pub size: Size<Pixels>,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            size: Size {
                width: px(820.0),
                height: px(900.0),
            },
        }
    }
}

impl WindowPreferences {
    pub fn new(
        width: impl Into<Pixels>,
        height: impl Into<Pixels>,
    ) -> Self {
        Self {
            size: Size {
                width: width.into(),
                height: height.into(),
            },
        }
    }
}

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

// ---------------------------------------------------------------------------
// Layout constants — SE Worksheet dialog (fixed-width columns)
// ---------------------------------------------------------------------------

/// Label column width for the SE Worksheet fixed layout.
pub const SE_LABEL_WIDTH: f32 = 250.0;

/// Field (input/display) column width for the SE Worksheet fixed layout.
pub const SE_FIELD_WIDTH: f32 = 150.0;

// ---------------------------------------------------------------------------
// InputState factories
// ---------------------------------------------------------------------------

/// Creates a currency-style [`InputState`] with a thousands separator and the
/// given number of decimal places. Generic over the owning view type.
pub fn make_decimal_input<V: 'static>(
    placeholder: impl Into<SharedString>,
    decimals: usize,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<InputState> {
    let pattern = MaskPattern::Number {
        separator: Some('_'),
        fraction: Some(decimals),
    };
    cx.new(|closure_cx| {
        InputState::new(window, closure_cx)
            .mask_pattern(pattern)
            .placeholder(placeholder.into())
            .clean_on_escape()
            .multi_line(false)
    })
}

/// Creates an integer-only [`InputState`] (no separator, no fractional part).
/// Generic over the owning view type.
pub fn make_integer_input<V: 'static>(
    placeholder: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Entity<InputState> {
    let pattern = MaskPattern::Number {
        separator: None,
        fraction: Some(0),
    };
    cx.new(|closure_cx| {
        InputState::new(window, closure_cx)
            .mask_pattern(pattern)
            .placeholder(placeholder.into())
            .clean_on_escape()
            .multi_line(false)
    })
}

// ---------------------------------------------------------------------------
// Flexible row builders (EstimatedIncomeForm — fills available width)
// ---------------------------------------------------------------------------

/// A labeled row with a flexible-width input that grows to fill space.
/// Matches the original EstimatedIncomeForm styling.
pub fn make_input_row(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
) -> Div {
    make_labeled_row(label).child(Input::new(state).flex_grow())
}

/// A labeled row containing a [`Select`] or any other already-rendered element.
pub fn make_select_row(
    label: impl Into<SharedString>,
    element: impl IntoElement,
) -> Div {
    make_labeled_row(label).child(element)
}

/// Base row: right-aligned label with a minimum width, border, and gap.
/// Matches the original EstimatedIncomeForm row style exactly.
pub fn make_labeled_row(label: impl Into<SharedString>) -> Div {
    h_flex()
        .items_center()
        .gap_5()
        .p(px(2.))
        .rounded_md()
        .border_1()
        .child(
            div()
                .min_w(px(150.))
                .text_align(TextAlign::Right)
                .child(label.into()),
        )
}

/// A full-width section heading row with an accent border and text color.
pub fn make_header_row(header: impl Into<SharedString>) -> Div {
    h_flex()
        .items_center()
        .justify_center()
        .p(px(4.))
        .mb_2()
        .child(
            div()
                .border_1()
                .px_4()
                .py_2()
                .rounded_md()
                .border_color(theme::HEADER_ACCENT)
                .text_color(theme::HEADER_ACCENT)
                .child(header.into()),
        )
}

// ---------------------------------------------------------------------------
// Fixed-width row builders (SeWorksheetForm dialog)
// ---------------------------------------------------------------------------

/// A labeled row with a fixed-width input. For use in fixed-layout dialogs
/// like the SE Worksheet where columns should not flex.
pub fn make_input_row_fixed(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
) -> Div {
    make_labeled_row_fixed(label).child(Input::new(state).w(px(SE_FIELD_WIDTH)))
}

/// A labeled row containing a read-only calculated value, fixed width.
/// Displays `"—"` when `value` is `None`.
pub fn make_display_row(
    label: impl Into<SharedString>,
    value: Option<Decimal>,
) -> Div {
    let display = value
        .map(|d| format!("${d:.2}"))
        .unwrap_or_else(|| "—".to_string());

    make_labeled_row_fixed(label).child(
        div()
            .w(px(SE_FIELD_WIDTH))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(theme::DISPLAY_FIELD_BORDER)
            .bg(theme::DISPLAY_FIELD_BG)
            .text_color(theme::DISPLAY_FIELD_TEXT)
            .text_align(TextAlign::Right)
            .child(display),
    )
}

/// Base row for fixed-layout dialogs: fixed-width right-aligned label,
/// no outer border (the individual fields carry their own borders).
pub fn make_labeled_row_fixed(label: impl Into<SharedString>) -> Div {
    h_flex().items_center().gap_2().p(px(2.)).child(
        div()
            .w(px(SE_LABEL_WIDTH))
            .text_align(TextAlign::Right)
            .flex_grow()
            .child(label.into()),
    )
}
