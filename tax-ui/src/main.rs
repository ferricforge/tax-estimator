use clap::Parser;
use gpui::{
    App, AppContext, Application, Bounds, Context, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowHandle, WindowOptions,
};

use gpui_component::Root;
use gpui_component_assets::Assets;
use tracing::{debug, info};

use tax_core::db::DbConfig;
use tax_ui::{
    app,
    components::{AppWindow, WindowPreferences},
    gui::build_main_content,
    logging::{init_default_logging, log_task_error},
    setup_app,
};

// ─── CLI definition ──────────────────────────────────────────────────────────

/// Estimated tax calculator for IRS Form 1040-ES.
///
/// Connects to the configured database, loads reference data for the
/// requested tax year, and prints it.
#[derive(Debug, Parser)]
struct Cli {
    /// Database backend to use.
    #[arg(long, default_value = "sqlite")]
    backend: String,

    /// Database connection string.
    /// For SQLite this is a file path (e.g. `taxes.db`) or `:memory:`.
    #[arg(long, default_value = "taxes.db")]
    db: String,

    /// Tax year to retrieve and display.
    #[arg(long, default_value = "2025")]
    year: i32,

    /// Run the UI
    #[arg(long)]
    ui: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_default_logging();

    let cli = Cli::parse();

    if cli.ui {
        run_ui();
        return Ok(());
    }

    let db_config = DbConfig {
        backend: cli.backend,
        connection_string: cli.db,
    };

    debug!("connecting to {} backend", db_config.backend);
    let registry = app::build_registry();
    let repo = registry.create(&db_config).await?;

    let data = app::load_tax_year_data(&*repo, cli.year).await?;
    info!("{}", data);

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
