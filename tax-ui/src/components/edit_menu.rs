//! The Edit menu.
//!
//! Undo and Redo are placeholders: their menu items are shown disabled until
//! an editing feature handles them. On Linux and Windows, the Edit menu also
//! holds Preferences. On macOS, Preferences is in the application menu.

use gpui::IntoElement;
#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};
use gpui_component::{
    Sizable,
    button::{Button, ButtonVariants},
    menu::DropdownMenu,
};

use super::preferences::{OpenPreferences, PREFERENCES_LABEL};

gpui::actions!(tax_estimator, [Undo, Redo]);

/// Builds the in-window Edit menu for Linux and Windows.
pub fn build_edit_menu_button() -> impl IntoElement {
    Button::new("edit-menu")
        .label("Edit")
        .ghost()
        .xsmall()
        .dropdown_menu(|menu, _window, _cx| {
            menu.menu_with_disabled("Undo", Box::new(Undo), true)
                .menu_with_disabled("Redo", Box::new(Redo), true)
                .separator()
                .menu(PREFERENCES_LABEL, Box::new(OpenPreferences))
        })
}

/// The Edit menu for the native macOS menu bar.
#[cfg(target_os = "macos")]
pub fn edit_app_menu() -> Menu {
    Menu {
        name: "Edit".into(),
        items: vec![
            MenuItem::action("Undo", Undo),
            MenuItem::action("Redo", Redo),
        ],
    }
}
