pub mod app;
pub mod components;
pub mod config;
pub mod csv_loader;
mod instructions;
pub mod logging;
pub mod models;
pub mod repository;
pub mod themes;
pub mod utils;

use gpui::KeyBinding;
use gpui::{App, actions};

#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem};

use tracing::info;

#[cfg(target_os = "macos")]
use crate::components::{LoadEstimate, NewProject, OpenProject, SaveProject, SaveProjectAs};
use crate::components::{bind_menu_keys, init_theme_colors};
use crate::config::{AppConfig, TomlConfigStore};
use crate::repository::ActiveTaxYear;
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

pub fn setup_app(app_cx: &mut App) {
    init_config(app_cx);

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

    // New / Open / Save / Save As are handled by `AppWindow` so the handlers
    // can reach the estimate form; see its `on_action` listeners in `render`.
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
                MenuItem::action("Load Estimate", LoadEstimate),
                MenuItem::separator(),
                MenuItem::action("Save", SaveProject),
                MenuItem::action("Save As...", SaveProjectAs),
            ],
        },
    ]);
    app_cx.activate(true);
}

fn init_config(cx: &mut App) {
    match TomlConfigStore::default_location() {
        Ok(store) => {
            tracing::info!("Config path: {}", store.path().display());
            if let Err(e) = AppConfig::init(cx, store) {
                tracing::error!("Failed to load config: {e:#}; using defaults");
                cx.set_global(AppConfig::default());
            }
        }
        Err(e) => {
            tracing::error!("Could not resolve config path: {e:#}; using in-memory defaults");
            cx.set_global(AppConfig::default());
        }
    }

    let cfg = AppConfig::get(cx);
    tracing::info!(
        database_url = %cfg.database_url,
        backend = %cfg.database_backend,
        "Configuration loaded"
    );
}
