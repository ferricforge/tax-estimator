use gpui::{
    App, AppContext, Application, Bounds, Context, Pixels, Size, TitlebarOptions,
    WindowBounds, WindowDecorations, WindowHandle, WindowOptions,
};
#[cfg(target_os = "linux")]
use gpui::{Point, px};
use gpui_component::Root;
use gpui_component_assets::Assets;

use tax_ui::{
    components::{AppWindow, WindowPreferences},
    gui::build_main_content,
    logging::{init_default_logging, log_task_error},
    setup_app,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_default_logging();

    run_ui();

    Ok(())
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

/// Computes window bounds, handling WSL's combined multi-monitor virtual desktop.
///
/// On WSL, X11 reports all monitors as one giant display. If we detect an
/// ultra-wide aspect ratio (> 2.5:1), we assume it's a dual-monitor setup
/// and center the window on the left half instead of the combined desktop.
#[cfg(target_os = "linux")]
fn compute_window_bounds(
    size: Size<Pixels>,
    app_cx: &App,
) -> Bounds<Pixels> {
    let is_wsl =
        std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some();

    let displays = app_cx.displays();
    let primary = displays.first();

    match primary {
        Some(display) => {
            let display_bounds = display.bounds();
            let display_size = display_bounds.size;

            // Aspect ratio > 2.5:1 suggests a combined multi-monitor desktop.
            // Normal ultra-wide monitors are 21:9 (2.33:1); 32:9 (3.56:1) is rare.
            // Two side-by-side 16:9 monitors = 32:9 combined.
            let is_ultra_wide = display_size.width > display_size.height * 2.5;

            if is_wsl && is_ultra_wide {
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

            // Normal case: center on this display
            Bounds::centered(Some(display.id()), size, app_cx)
        }
        None => {
            // No display found; fall back to default
            Bounds::centered(None, size, app_cx)
        }
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

fn run_ui() {
    init_default_logging();

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
        setup_app(app_cx);

        let prefs = WindowPreferences::default();

        let titlebar = Some(TitlebarOptions {
            title: Some("TimeKeeper Loader".into()),
            appears_transparent: false,
            ..Default::default()
        });

        app_cx
            .spawn(async move |async_cx| {
                let result: anyhow::Result<()> = async {
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
                                let content = build_main_content(window, view_cx);
                                let mut main_window = AppWindow::new(view_cx);
                                main_window.set_content(content);
                                main_window
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
