pub mod app;
pub mod components;
pub mod csv_loader;
pub mod logging;
pub mod models;
pub mod themes;
pub mod utils;

use gpui::KeyBinding;
use gpui::{actions, Action, App};

#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};

use tracing::info;

#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::themes::apply_macos_system_theme;
#[cfg(target_os = "windows")]
use crate::themes::apply_windows_system_theme;
use crate::{
    components::{
        CloseProject, NewProject, OpenProject, SaveProject, SaveProjectAs, bind_menu_keys,
        init_theme_colors,
    },
};


actions!(tax_estimator, [Quit]);

// Takes a reference to the action (often unused) and mutable app context
pub fn quit(
    _: &Quit,
    cx: &mut App,
) {
    info!("Executing quit handler");
    cx.quit();
}


/// Registers a handler for a GPUI [`Action`] type.
fn register_action<A: Action>(
    app: &mut App,
    f: impl Fn(&A, &mut App) + 'static,
) {
    app.on_action(f);
}

fn stub_file_action<A: Action>(name: &'static str) -> impl Fn(&A, &mut App) {
    move |_, _| {
        tracing::info!("{name}: not yet implemented");
    }
}

pub fn setup_app(app_cx: &mut App) {
    gpui_component::init(app_cx);

    #[cfg(target_os = "macos")]
    apply_macos_system_theme(app_cx);
    #[cfg(target_os = "windows")]
    apply_windows_system_theme(app_cx);
    #[cfg(target_os = "linux")]
    apply_linux_system_theme(app_cx);

    // Populate legacy theme constants from the now-active theme.
    init_theme_colors(app_cx);

    #[cfg(target_os = "macos")]
    app_cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

    #[cfg(not(target_os = "macos"))]
    app_cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("alt-F4", Quit, None),
    ]);

    app_cx.on_action(quit);

    register_action(app_cx, stub_file_action::<NewProject>("NewProject"));
    register_action(app_cx, stub_file_action::<OpenProject>("OpenProject"));
    register_action(app_cx, stub_file_action::<SaveProject>("SaveProject"));
    register_action(app_cx, stub_file_action::<SaveProjectAs>("SaveProjectAs"));
    register_action(app_cx, stub_file_action::<CloseProject>("CloseProject"));

    bind_menu_keys(app_cx);

    // Native macOS menu bar
    #[cfg(target_os = "macos")]
    app_cx.set_menus(vec![
        Menu {
            name: "Tax Estimator".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Project", NewProject),
                MenuItem::action("Open Project", OpenProject),
                MenuItem::separator(),
                MenuItem::action("Save", SaveProject),
                MenuItem::action("Save As...", SaveProjectAs),
                MenuItem::separator(),
                MenuItem::action("Close Project", CloseProject),
            ],
        },
    ]);
    app_cx.activate(true);
}
