pub mod components;
pub mod config;
mod connection;
pub mod csv_loader;
pub mod estimate;
mod file_dialogs;
mod instructions;
pub mod logging;
pub mod models;
pub mod repository;
pub mod session;
pub mod startup;
pub mod state;
pub mod themes;
pub mod utils;

use gpui::KeyBinding;
use gpui::{App, actions};
use tracing::info;

#[cfg(target_os = "macos")]
use crate::components::build_app_menus;
use crate::components::{
    bind_menu_keys, bind_preferences_keys, init_theme_colors, open_preferences,
    save_tracked_window_bounds,
};
use crate::config::{AppConfig, ConfigStore};
use crate::state::ActiveTaxYear;
#[cfg(target_os = "linux")]
use crate::themes::apply_linux_system_theme;
#[cfg(target_os = "macos")]
use crate::themes::apply_macos_system_theme;
#[cfg(target_os = "windows")]
use crate::themes::apply_windows_system_theme;

actions!(tax_estimator, [Quit]);

// Takes a reference to the action (often unused) and mutable app context
pub fn quit(
    _: &Quit,
    cx: &mut App,
) {
    info!("Executing quit handler");
    cx.quit();
}

/// Installs the native macOS menu bar from the current configuration.
#[cfg(target_os = "macos")]
fn install_macos_app_menus(cx: &mut App) {
    let menus = build_app_menus(cx);
    cx.set_menus(menus);
}

pub fn setup_app(
    app_cx: &mut App,
    config: AppConfig,
    config_store: Option<Box<dyn ConfigStore>>,
) {
    AppConfig::install(app_cx, config, config_store);

    // Placeholder so observe_global has something to attach to before the
    // async repo init finishes.
    app_cx.set_global(ActiveTaxYear::default());

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

    // New / Open / Save / Save As and the Recent entries are handled by
    // `AppWindow` so the handlers can reach the estimate form; see its
    // `on_action` listeners in `render`.
    bind_menu_keys(app_cx);

    // Preferences opens from the application menu (macOS) or the Edit menu
    // (elsewhere), and from its shortcut.
    bind_preferences_keys(app_cx);
    app_cx.on_action(open_preferences);

    // Quitting does not send windows a close request, so their geometry is
    // saved here instead of in each window's close hook.
    app_cx
        .on_app_quit(|app_cx: &mut App| {
            save_tracked_window_bounds(app_cx);
            std::future::ready(())
        })
        .detach();

    // Native macOS menu bar. It is rebuilt whenever the configuration
    // changes, so the Recent submenu follows the recent list.
    #[cfg(target_os = "macos")]
    {
        install_macos_app_menus(app_cx);
        app_cx
            .observe_global::<AppConfig>(install_macos_app_menus)
            .detach();
    }

    app_cx.activate(true);
}
