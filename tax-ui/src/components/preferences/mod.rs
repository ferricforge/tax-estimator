//! The preferences window.
//!
//! On macOS the *Settings...* item in the application menu opens the window.
//! Elsewhere the *Preferences...* item in the Edit menu opens it. The shortcut
//! is `cmd-,` on macOS and `ctrl-,` elsewhere. Only one preferences window is
//! open at a time.

mod window;

use gpui::{
    App, AppContext, Bounds, KeyBinding, Pixels, Size, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, actions, px,
};
use gpui_component::Root;

use crate::components::{restore_saved_window_bounds, track_window, tracked_window};
use crate::config::PREFERENCES_WINDOW;

pub use window::PreferencesWindow;

actions!(tax_estimator, [OpenPreferences]);

/// Menu label of the item that opens the window, on macOS.
#[cfg(target_os = "macos")]
pub const PREFERENCES_LABEL: &str = "Settings...";

/// Menu label of the item that opens the window, on Linux and Windows.
#[cfg(not(target_os = "macos"))]
pub const PREFERENCES_LABEL: &str = "Preferences...";

/// Title shown in the window's title bar.
const TITLE: &str = "Preferences";

/// Size used when no geometry has been remembered.
fn default_size() -> Size<Pixels> {
    Size {
        width: px(560.0),
        height: px(520.0),
    }
}

/// Registers the keyboard shortcut that opens the window.
pub fn bind_preferences_keys(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.bind_keys([KeyBinding::new("cmd-,", OpenPreferences, None)]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([KeyBinding::new("ctrl-,", OpenPreferences, None)]);
}

/// Opens the preferences window, or brings the open one to the front.
pub fn open_preferences(
    _: &OpenPreferences,
    cx: &mut App,
) {
    if let Some(handle) = tracked_window(PREFERENCES_WINDOW, cx) {
        let activated = handle.update(cx, |_, window, _| window.activate_window());
        if activated.is_ok() {
            return;
        }
    }

    let bounds = restore_saved_window_bounds(PREFERENCES_WINDOW, cx)
        .unwrap_or_else(|| Bounds::centered(None, default_size(), cx));

    let titlebar = Some(TitlebarOptions {
        title: Some(TITLE.into()),
        appears_transparent: false,
        ..Default::default()
    });

    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar,
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        },
        |window: &mut gpui::Window, window_cx| {
            let view = window_cx.new(|view_cx: &mut gpui::Context<PreferencesWindow>| {
                PreferencesWindow::new(window, view_cx)
            });
            window_cx.new(|root_cx| Root::new(view, window, root_cx))
        },
    );

    match result {
        Ok(handle) => track_window(PREFERENCES_WINDOW, handle.into(), cx),
        Err(error) => tracing::error!(%error, "could not open the preferences window"),
    }
}
