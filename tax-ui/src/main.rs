use gpui::{
    App, AppContext, Application, Bounds, Context, Pixels, Size, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowHandle, WindowOptions,
};
#[cfg(target_os = "linux")]
use gpui::{Point, px};
use gpui_component::Root;
use gpui_component_assets::Assets;
use std::path::PathBuf;

use tax_ui::{
    components::{AppWindow, WindowPreferences},
    config::{AppConfig, ConfigStore, TomlConfigStore},
    logging::{apply_settings, init_default_logging, log_task_error},
    setup_app,
    startup::{StartupOutcome, open_database_or_ask},
};

/// Summary used when logging setup does not complete.
const LOGGING_SETUP_WARNING: &str = "logging setup incomplete";

/// Configuration and persistence state resolved before logging and gpui start.
struct StartupConfig {
    config: AppConfig,
    store: Option<Box<dyn ConfigStore>>,
    path: Option<PathBuf>,
    created: bool,
    removed_recent: usize,
    error: Option<StartupConfigError>,
}

/// Stage at which startup configuration loading failed.
enum StartupConfigError {
    ResolvePath(anyhow::Error),
    Load(anyhow::Error),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let startup_config = load_startup_config();

    if let Err(error) = init_default_logging() {
        report_logging_setup_error(&error);
    }

    if let Err(error) = apply_settings(&startup_config.config.logging.settings()) {
        tracing::error!("Failed to apply logging settings: {error:#}");
    }

    report_startup_config(&startup_config);
    run_ui(startup_config);

    Ok(())
}

/// Loads the application configuration before logging and gpui initialization.
///
/// Recent connections whose files no longer exist are removed from the
/// loaded list; the shorter list is written the next time the configuration
/// is saved.
///
/// Failures retain an in-memory default configuration. They are reported only
/// after logging is installed.
fn load_startup_config() -> StartupConfig {
    let store = match TomlConfigStore::default_location() {
        Ok(store) => store,
        Err(error) => {
            return StartupConfig {
                config: AppConfig::default(),
                store: None,
                path: None,
                created: false,
                removed_recent: 0,
                error: Some(StartupConfigError::ResolvePath(error)),
            };
        }
    };
    let path = store.path().to_path_buf();
    let created = !store.exists();

    match store.load_or_init() {
        Ok(mut config) => {
            let removed_recent = config.recent.remove_missing();
            StartupConfig {
                config,
                store: Some(Box::new(store)),
                path: Some(path),
                created,
                removed_recent,
                error: None,
            }
        }
        Err(error) => StartupConfig {
            config: AppConfig::default(),
            store: None,
            path: Some(path),
            created: false,
            removed_recent: 0,
            error: Some(StartupConfigError::Load(error)),
        },
    }
}

/// Reports configuration loading after its logging settings are active.
fn report_startup_config(startup: &StartupConfig) {
    match startup.error.as_ref() {
        Some(StartupConfigError::ResolvePath(error)) => {
            tracing::error!("Could not resolve config path: {error:#}; using in-memory defaults");
        }
        Some(StartupConfigError::Load(error)) => {
            tracing::error!("Failed to load config: {error:#}; using defaults");
        }
        None => {}
    }

    if startup.created {
        tracing::info!("No existing config found; wrote defaults");
    }

    if let Some(path) = startup.path.as_ref() {
        tracing::info!("Config path: {}", path.display());
    }

    if startup.removed_recent > 0 {
        tracing::info!(
            removed = startup.removed_recent,
            "Removed recent connections whose files no longer exist"
        );
    }

    let database = &startup.config.database;
    tracing::info!(
        database_url = %database.url,
        backend = %database.backend,
        "Configuration loaded"
    );
}

/// Reports a logging setup failure without stopping the application.
///
/// The report goes to stderr, which is visible when the application runs from
/// a terminal. It is also emitted as a `tracing` event: after a setup failure
/// a subscriber is usually still active (another subscriber, or this
/// application's own when only the `log` bridge failed), so the event is
/// recorded there as well.
fn report_logging_setup_error(error: &anyhow::Error) {
    eprintln!("Warning: {LOGGING_SETUP_WARNING}: {error:#}");
    tracing::warn!(?error, "{LOGGING_SETUP_WARNING}");
}

#[cfg(target_os = "linux")]
fn should_force_xwayland() -> bool {
    let is_wsl =
        std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some();

    // In WSL, always force XWayland — the WSLg Wayland compositor is too
    // old for gpui regardless of which display variables happen to be set.
    if is_wsl {
        return true;
    }

    // On native Linux, only nudge GNOME toward XWayland when both display
    // servers are available (same behaviour as before).
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false);

    let has_x11_display = std::env::var_os("DISPLAY").is_some();
    let has_wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();

    is_gnome && has_x11_display && has_wayland_display
}

/// Computes window bounds for Linux.
///
/// **Native Linux** always uses [`Bounds::centered`]; no custom sizing or
/// positioning logic runs outside WSL.
///
/// **WSL only:** X11 can report all monitors as one combined desktop. If the
/// primary display looks ultra-wide (aspect ratio > 2.5:1), we assume a
/// dual-monitor span and center the window on the **left half** instead of the
/// full virtual desktop. Otherwise WSL uses [`Bounds::centered`] like native
/// Linux.
#[cfg(target_os = "linux")]
fn compute_window_bounds(
    size: Size<Pixels>,
    app_cx: &App,
) -> Bounds<Pixels> {
    let is_wsl =
        std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some();

    let displays = app_cx.displays();
    let primary = displays.first();

    if !is_wsl {
        return match primary {
            Some(display) => Bounds::centered(Some(display.id()), size, app_cx),
            None => Bounds::centered(None, size, app_cx),
        };
    }

    match primary {
        Some(display) => {
            let display_bounds = display.bounds();
            let display_size = display_bounds.size;

            // Aspect ratio > 2.5:1 suggests a combined multi-monitor desktop.
            // Normal ultra-wide monitors are 21:9 (2.33:1); 32:9 (3.56:1) is rare.
            // Two side-by-side 16:9 monitors = 32:9 combined.
            let is_ultra_wide = display_size.width > display_size.height * 2.5;

            if is_ultra_wide {
                tracing::debug!(
                    "WSL ultra-wide detected ({} x {}), centering on left half",
                    display_size.width,
                    display_size.height
                );

                // Assume dual monitors: center on left half
                let half_width = display_size.width / 2.0;

                let x = display_bounds.origin.x + (half_width - size.width) / 2.0;
                let y = display_bounds.origin.y + (display_size.height - size.height) / 2.0;

                // Clamp to non-negative
                let x = if x < px(0.0) { px(0.0) } else { x };
                let y = if y < px(0.0) { px(0.0) } else { y };

                return Bounds {
                    origin: Point { x, y },
                    size,
                };
            }

            Bounds::centered(Some(display.id()), size, app_cx)
        }
        None => Bounds::centered(None, size, app_cx),
    }
}

/// Non-Linux platforms: just use standard centering.
#[cfg(not(target_os = "linux"))]
fn compute_window_bounds(
    size: Size<Pixels>,
    app_cx: &App,
) -> Bounds<Pixels> {
    Bounds::centered(None, size, app_cx)
}

fn run_ui(startup_config: StartupConfig) {
    #[cfg(target_os = "linux")]
    {
        if should_force_xwayland() {
            let is_wsl = std::env::var_os("WSL_DISTRO_NAME").is_some()
                || std::env::var_os("WSL_INTEROP").is_some();

            // SAFETY: Single-threaded startup; no worker threads exist yet.
            unsafe {
                std::env::remove_var("WAYLAND_DISPLAY");

                // WSLg exposes an X11 socket at :0 even when the user has
                // unset DISPLAY. Put it back so gpui's X11 backend can find it.
                if is_wsl && std::env::var_os("DISPLAY").is_none() {
                    std::env::set_var("DISPLAY", ":0");
                }
            }

            tracing::info!(
                "Wayland compatibility issue detected (WSL or GNOME/XWayland); \
                 forced XWayland (WAYLAND_DISPLAY unset, DISPLAY={})",
                std::env::var("DISPLAY").unwrap_or_else(|_| "<unset>".into())
            );
        }

        // After all display manipulation, verify we have a display server to connect to
        let has_display = std::env::var_os("DISPLAY").is_some();
        let has_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();

        if !has_display && !has_wayland {
            eprintln!("Error: no DISPLAY or WAYLAND_DISPLAY environment variable specified");
            std::process::exit(1);
        }
    }

    let app = Application::new().with_assets(Assets);

    app.run(move |app_cx: &mut App| {
        setup_app(app_cx, startup_config.config, startup_config.store);

        let prefs = WindowPreferences::default();

        let titlebar = Some(TitlebarOptions {
            title: Some("Tax Estimator".into()),
            appears_transparent: false,
            ..Default::default()
        });

        app_cx
            .spawn(async move |async_cx| {
                let result: anyhow::Result<()> = async {
                    if open_database_or_ask(async_cx).await? == StartupOutcome::Declined {
                        async_cx.update(|app_cx: &mut App| app_cx.quit())?;
                        return Ok(());
                    }

                    let bounds = async_cx
                        .update(|app_cx: &mut App| compute_window_bounds(prefs.size, app_cx))?;

                    let _window_handle: WindowHandle<Root> = async_cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                            titlebar,
                            window_decorations: Some(WindowDecorations::Server),
                            ..Default::default()
                        },
                        |window: &mut gpui::Window, window_cx| {
                            let view = window_cx.new(|view_cx: &mut Context<AppWindow>| {
                                AppWindow::new(window, view_cx)
                            });
                            window_cx.new(|root_cx| Root::new(view, window, root_cx))
                        },
                    )?;

                    Ok(())
                }
                .await;

                log_task_error("main_window_setup", result);
                Ok::<_, anyhow::Error>(())
            })
            .detach();
    });
}
