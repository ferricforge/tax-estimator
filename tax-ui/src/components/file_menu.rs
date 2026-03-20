// file_menu.rs
use gpui::{App, KeyBinding, ParentElement, Styled};
use gpui_component::{
    IconName, Sizable, button::{Button, ButtonVariants}, h_flex, menu::DropdownMenu
};

use crate::Quit; // reuse the app-wide action

// Add any new actions you need
gpui::actions!(timekeeper, [NewProject, OpenProject, SaveProject, SaveProjectAs, CloseProject]);


pub fn bind_menu_keys(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewProject, None),
        KeyBinding::new("cmd-o", OpenProject, None),
        KeyBinding::new("cmd-s", SaveProject, None),
        KeyBinding::new("cmd-shift-s", SaveProjectAs, None),
        KeyBinding::new("cmd-w", CloseProject, None),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-n", NewProject, None),
        KeyBinding::new("ctrl-o", OpenProject, None),
        KeyBinding::new("ctrl-s", SaveProject, None),
        KeyBinding::new("ctrl-shift-s", SaveProjectAs, None),
        KeyBinding::new("ctrl-w", CloseProject, None),
    ]);
}

/// Builds an in-window menu bar for Linux/Windows.
pub fn build_menu_bar() -> impl gpui::IntoElement {
    h_flex()
        .gap_0()
        .child(
            Button::new("file-menu")
                .label("File")
                .ghost()
                .xsmall()
                .dropdown_menu(|menu, _window, _cx| {
                    menu.menu_with_icon("New Project", IconName::File, Box::new(NewProject))
                        .menu_with_icon("Open Project", IconName::FolderOpen, Box::new(OpenProject))
                        .separator()
                        .menu("Save", Box::new(SaveProject))
                        .menu("Save As...", Box::new(SaveProjectAs))
                        .separator()
                        .menu("Close Project", Box::new(CloseProject))
                        .separator()
                        .menu("Quit", Box::new(Quit))
                }),
        )
    // .child(Button::new("edit-menu").label("Edit").ghost().xsmall().dropdown_menu(...))
    // .child(Button::new("help-menu").label("Help").ghost().xsmall().dropdown_menu(...))
}