use gpui::{
    App, AppContext, Application, Bounds, Context, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowHandle, WindowOptions,
};

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

fn run_ui() {
    init_default_logging();

    #[cfg(target_os = "linux")]
    {
        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_ascii_lowercase().contains("gnome"))
            .unwrap_or(false);
        let has_x11_display = std::env::var_os("DISPLAY").is_some();
        let has_wayland_display = std::env::var_os("WAYLAND_DISPLAY").is_some();

        if is_gnome && has_x11_display && has_wayland_display {
            tracing::info!(
                "GNOME Wayland detected; falling back to XWayland for window decorations"
            );
            // SAFETY: This runs at process startup before any worker threads
            // are started, so mutating process environment is confined to this
            // single-threaded initialization phase.
            unsafe {
                std::env::remove_var("WAYLAND_DISPLAY");
            }
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
                        .update(|app_cx: &mut App| Bounds::centered(None, prefs.size, app_cx))?;

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
