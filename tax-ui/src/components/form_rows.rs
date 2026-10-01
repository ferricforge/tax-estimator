use gpui::{
    App, Div, Entity, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, TextAlign, Window, div, px, relative,
};
use gpui_component::input::{Input, InputState};
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt, h_flex, v_flex};
use rust_decimal::Decimal;

use super::theme;
use crate::instructions::FieldHelp;

// ---------------------------------------------------------------------------
// Layout constants — SE Worksheet dialog (fixed-width columns)
// ---------------------------------------------------------------------------

/// Label column width for the SE Worksheet fixed layout.
pub const SE_LABEL_WIDTH: f32 = 250.0;
/// Field (input/display) column width for the SE Worksheet fixed layout.
pub const SE_FIELD_WIDTH: f32 = 150.0;
/// Width of the help-icon column, reserved on every row whether or not the
/// row has help text, so fields stay aligned.
const HELP_ICON_WIDTH: f32 = 24.0;

// ---------------------------------------------------------------------------
// Flexible row builders (EstimatedIncomeForm — fills available width)
// ---------------------------------------------------------------------------

/// A labeled row with a flexible-width input that grows to fill space.
/// Matches the original EstimatedIncomeForm styling.
pub fn make_input_row(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
) -> Div {
    make_input_row_with_help(state, label, None)
}

/// A labeled row with a flexible-width input and a help column. Reserves the
/// help column even when `help` is `None`.
pub(crate) fn make_input_row_with_help(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
    help: Option<FieldHelp>,
) -> Div {
    make_labeled_row_with_help(label, None)
        .child(Input::new(state).flex_grow())
        .child(make_help_slot(help))
}

/// A labeled row containing a [`Select`] or any other already-rendered element.
pub fn make_select_row(
    label: impl Into<SharedString>,
    element: impl IntoElement,
) -> Div {
    make_select_row_with_help(label, element, None)
}

/// A labeled row containing a [`Select`] or any other already-rendered
/// element, with a help column. Reserves the help column even when `help`
/// is `None`.
fn make_select_row_with_help(
    label: impl Into<SharedString>,
    element: impl IntoElement,
    help: Option<FieldHelp>,
) -> Div {
    make_labeled_row_with_help(label, None)
        .child(element)
        .child(make_help_slot(help))
}

/// Base row: right-aligned label with a minimum width, border, and gap.
/// Matches the original EstimatedIncomeForm row style exactly.
pub fn make_labeled_row(label: impl Into<SharedString>) -> Div {
    make_labeled_row_with_help(label, None)
}

/// Base row: right-aligned label with optional help tooltip.
fn make_labeled_row_with_help(
    label: impl Into<SharedString>,
    _help: Option<FieldHelp>,
) -> Div {
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
                .child(build_label_content(label.into())),
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
// Fixed-width row builders (SeWorksheetForm and QbiForm dialogs)
// ---------------------------------------------------------------------------

/// A labeled row with a fixed-width input. For use in fixed-layout dialogs
/// like the SE Worksheet where columns should not flex.
pub fn make_input_row_fixed(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
) -> Div {
    make_input_row_fixed_with_help(state, label, None)
}

/// A labeled row with a fixed-width input and a help column. Reserves the
/// help column even when `help` is `None`.
pub(crate) fn make_input_row_fixed_with_help(
    state: &Entity<InputState>,
    label: impl Into<SharedString>,
    help: Option<FieldHelp>,
) -> Div {
    make_labeled_row_fixed_with_help(label, None)
        .child(Input::new(state).w(px(SE_FIELD_WIDTH)))
        .child(make_help_slot(help))
}

/// A labeled row containing a read-only calculated value, fixed width.
/// Displays `"—"` when `value` is `None`.
pub fn make_display_row(
    label: impl Into<SharedString>,
    value: Option<Decimal>,
) -> Div {
    make_display_row_with_help(label, value, None)
}

/// A labeled row containing a read-only calculated value, fixed width, with
/// a help column. Reserves the help column even when `help` is `None`.
pub(crate) fn make_display_row_with_help(
    label: impl Into<SharedString>,
    value: Option<Decimal>,
    help: Option<FieldHelp>,
) -> Div {
    let display = value
        .map(|d| format!("${d:.2}"))
        .unwrap_or_else(|| "—".to_string());
    make_display_text_row_with_help(label, display, help)
}

/// A labeled row containing a read-only carryforward value, fixed width.
/// Negative values display in parentheses. Displays `"—"` when `value` is
/// `None`.
pub(crate) fn make_carryforward_display_row_with_help(
    label: impl Into<SharedString>,
    value: Option<Decimal>,
    help: Option<FieldHelp>,
) -> Div {
    let display = value
        .map(format_carryforward)
        .unwrap_or_else(|| "—".to_string());
    make_display_text_row_with_help(label, display, help)
}

/// Formats a carryforward amount, wrapping negative values in parentheses.
fn format_carryforward(value: Decimal) -> String {
    if value < Decimal::ZERO {
        format!("(${:.2})", value.abs())
    } else {
        format!("${value:.2}")
    }
}

/// Shared fixed-width display field used by the display row builders.
fn make_display_text_row_with_help(
    label: impl Into<SharedString>,
    display: String,
    help: Option<FieldHelp>,
) -> Div {
    make_labeled_row_fixed_with_help(label, None)
        .child(
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
        .child(make_help_slot(help))
}

/// Base row for fixed-layout dialogs: fixed-width right-aligned label,
/// no outer border (the individual fields carry their own borders).
pub fn make_labeled_row_fixed(label: impl Into<SharedString>) -> Div {
    make_labeled_row_fixed_with_help(label, None)
}

/// Base row for fixed-layout dialogs with optional help tooltip.
fn make_labeled_row_fixed_with_help(
    label: impl Into<SharedString>,
    _help: Option<FieldHelp>,
) -> Div {
    h_flex().items_center().gap_2().p(px(2.)).child(
        div()
            .w(px(SE_LABEL_WIDTH))
            .text_align(TextAlign::Right)
            .flex_grow()
            .child(build_label_content(label.into())),
    )
}

// ---------------------------------------------------------------------------
// Label and help-tooltip building blocks
// ---------------------------------------------------------------------------

fn build_label_content(label: SharedString) -> impl IntoElement {
    let tooltip_id = SharedString::from(format!(
        "field-help-{}",
        label
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            })
            .collect::<String>()
    ));
    h_flex()
        .id(tooltip_id)
        .items_center()
        .justify_end()
        .child(div().child(label))
}

/// Fixed-width column for a field's help icon. Reserves the same width
/// whether or not help is available, so fields across a form stay aligned.
pub(crate) fn make_help_slot(help: Option<FieldHelp>) -> Div {
    let slot = div().w(px(HELP_ICON_WIDTH)).flex_none().ml_1();
    match help {
        Some(help) => slot.child(build_help_icon(help)),
        None => slot,
    }
}

fn build_help_icon(help: FieldHelp) -> impl IntoElement {
    let tooltip_id = SharedString::from(format!(
        "field-help-icon-{}",
        help.label
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            })
            .collect::<String>()
    ));
    div()
        .id(tooltip_id)
        .py_0p5()
        .child(Icon::new(IconName::Info).size_4())
        .tooltip(move |window, cx| build_field_help_tooltip(&help, window, cx))
}

fn build_field_help_tooltip(
    help: &FieldHelp,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyView {
    let title = help.label.clone();
    let paragraphs = help.paragraphs.clone();
    Tooltip::element(move |_window, _cx| {
        v_flex()
            .gap_3()
            .w(px(420.))
            .child(
                div()
                    .w_full()
                    .font_semibold()
                    .whitespace_normal()
                    .line_height(relative(1.1))
                    .child(title.clone()),
            )
            .children(paragraphs.iter().cloned().map(|paragraph| {
                div()
                    .w_full()
                    .whitespace_normal()
                    .line_height(relative(1.25))
                    .child(paragraph)
            }))
    })
    .border_color(cx.theme().warning)
    .build(window, cx)
}
