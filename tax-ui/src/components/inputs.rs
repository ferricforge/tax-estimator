use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::input::{InputState, MaskPattern};
use rust_decimal::Decimal;

use crate::utils::optional_decimal_input_text;

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

/// Writes a text value into an input's [`InputState`], for any sync context.
pub(crate) fn set_input_value<C: AppContext>(
    input: &Entity<InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut C,
) {
    let value = value.into();
    input.update(cx, |state, is_cx| {
        state.set_value(value, window, is_cx);
    });
}

/// Writes a [`Decimal`] value into an input's [`InputState`].
pub(crate) fn set_decimal_input<C: AppContext>(
    input: &Entity<InputState>,
    value: Decimal,
    window: &mut Window,
    cx: &mut C,
) {
    set_input_value(input, value.to_string(), window, cx);
}

/// Writes an optional [`Decimal`] into an input's [`InputState`], clearing the
/// field when the value is `None`.
pub(crate) fn set_optional_decimal_input<C: AppContext>(
    input: &Entity<InputState>,
    value: Option<Decimal>,
    window: &mut Window,
    cx: &mut C,
) {
    set_input_value(input, optional_decimal_input_text(value), window, cx);
}
