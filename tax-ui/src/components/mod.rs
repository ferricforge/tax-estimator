pub mod dialogs;
pub mod estimate_form;
pub mod window;

use gpui::{App, SharedString, Window};
use gpui::{ClickEvent, Styled};
use gpui::{Pixels, Size, px};
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};

pub use estimate_form::EstimatedIncomeForm;
pub use window::AppWindow;

#[derive(Debug, Clone, Copy)]
pub struct WindowPreferences {
    pub size: Size<Pixels>,
    // TODO: Implement window centering once we determine the correct
    // gpui API for getting display bounds
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
        .w(px(140.)) // ← fixed width
        .label(label.into())
        .on_click(on_click)
}
