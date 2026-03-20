pub mod dialogs;
mod estimate_form;
pub mod file_picker;
pub mod filters;
mod form_helpers;
mod se_worksheet_form;
pub(crate) mod theme;
pub mod window;

use gpui::{App, SharedString, Window};
use gpui::{ClickEvent, Styled};
use gpui::{Pixels, Size, px};
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};

pub use dialogs::ErrorDialog;
pub use estimate_form::EstimatedIncomeForm;
pub use form_helpers::{
    SE_FIELD_WIDTH, SE_LABEL_WIDTH, make_decimal_input, make_display_row, make_header_row,
    make_input_row, make_input_row_fixed, make_integer_input, make_labeled_row,
    make_labeled_row_fixed, make_select_row,
};
pub use se_worksheet_form::SeWorksheetForm;
pub use window::AppWindow;

#[derive(Debug, Clone, Copy)]
pub struct WindowPreferences {
    pub size: Size<Pixels>,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            size: Size {
                width: px(800.0),
                height: px(800.0),
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
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id.into())
        .primary()
        .large()
        .w(px(140.))
        .label(label.into())
        .on_click(on_click)
}
