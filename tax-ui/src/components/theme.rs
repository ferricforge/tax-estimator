use std::sync::{PoisonError, RwLock};

use gpui::{App, Hsla};
use gpui_component::Theme;

use crate::config::AppConfig;
use crate::themes::{apply_theme, install_appearance};

/// An index into the cached theme colors.
///
/// This type is `Copy`, allowing it to be used like a constant while
/// the actual color value is set from the active theme at runtime.
/// Implements `From<ThemeColor>` for both `Hsla` and `gpui::Fill`, so it
/// works directly with GPUI styling methods like `.bg()`, `.text_color()`, etc.
#[derive(Copy, Clone)]
pub struct ThemeColor(usize);

/// Message used when a color is read before [`init_theme_colors`] has run.
const NOT_INITIALIZED: &str =
    "theme colors not initialized; call init_theme_colors() in setup_app()";

/// Cached theme colors. [`init_theme_colors`] rewrites them whenever the
/// appearance changes, so every window drawn afterwards uses the new colors.
static THEME_COLORS: RwLock<[Option<Hsla>; 4]> = RwLock::new([None; 4]);

const IDX_DISPLAY_FIELD_BG: usize = 0;
const IDX_DISPLAY_FIELD_BORDER: usize = 1;
const IDX_DISPLAY_FIELD_TEXT: usize = 2;
const IDX_HEADER_ACCENT: usize = 3;

// ---------------------------------------------------------------------------
// Display field colors
// ---------------------------------------------------------------------------

/// Background for read-only calculated display fields.
pub const DISPLAY_FIELD_BG: ThemeColor = ThemeColor(IDX_DISPLAY_FIELD_BG);

/// Border color for read-only calculated display fields.
pub const DISPLAY_FIELD_BORDER: ThemeColor = ThemeColor(IDX_DISPLAY_FIELD_BORDER);

/// Text color for read-only calculated display fields.
pub const DISPLAY_FIELD_TEXT: ThemeColor = ThemeColor(IDX_DISPLAY_FIELD_TEXT);

// ---------------------------------------------------------------------------
// Section header colors
// ---------------------------------------------------------------------------

/// Border and text color for section header rows.
pub const HEADER_ACCENT: ThemeColor = ThemeColor(IDX_HEADER_ACCENT);

// ---------------------------------------------------------------------------
// ThemeColor implementation
// ---------------------------------------------------------------------------

impl ThemeColor {
    fn get_hsla(self) -> Hsla {
        let slots = THEME_COLORS.read().unwrap_or_else(PoisonError::into_inner);
        slots[self.0].expect(NOT_INITIALIZED)
    }
}

impl From<ThemeColor> for Hsla {
    fn from(color: ThemeColor) -> Hsla {
        color.get_hsla()
    }
}

impl From<ThemeColor> for gpui::Fill {
    fn from(color: ThemeColor) -> gpui::Fill {
        gpui::Fill::from(color.get_hsla())
    }
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Reads the colors of the active theme into the cache used by [`ThemeColor`].
///
/// Call this after `gpui_component::init(cx)` and after the platform theme has
/// been applied. [`apply_configured_theme`] does both.
pub fn init_theme_colors(cx: &App) {
    let colors = &Theme::global(cx).colors;
    let mut slots = THEME_COLORS.write().unwrap_or_else(PoisonError::into_inner);

    slots[IDX_DISPLAY_FIELD_BG] = Some(colors.muted);
    slots[IDX_DISPLAY_FIELD_BORDER] = Some(colors.border);
    slots[IDX_DISPLAY_FIELD_TEXT] = Some(colors.muted_foreground);
    slots[IDX_HEADER_ACCENT] = Some(colors.primary);
}

/// Applies the appearance chosen in the configuration, now.
///
/// Use this during startup, before any window exists. The cached colors must
/// be filled before the first render, so this cannot be deferred.
pub fn apply_configured_theme(cx: &mut App) {
    let mode = AppConfig::get(cx).appearance.theme;

    install_appearance(mode);
    apply_theme(cx, mode);
    init_theme_colors(cx);
    cx.refresh_windows();
}

/// Applies the appearance chosen in the configuration once the current update
/// has finished.
///
/// Use this while windows are open. The appearance is installed from a spawned
/// task so that no `App` borrow is held: on macOS the window system notifies
/// gpui during that call, and gpui borrows the application to handle it.
pub fn reapply_configured_theme(cx: &mut App) {
    let mode = AppConfig::get(cx).appearance.theme;

    cx.spawn(async move |async_cx| {
        install_appearance(mode);

        let result = async_cx.update(|app_cx: &mut App| {
            apply_theme(app_cx, mode);
            init_theme_colors(app_cx);
            app_cx.refresh_windows();
        });

        if let Err(error) = result {
            tracing::error!(%error, "failed to apply the theme");
        }
    })
    .detach();
}
