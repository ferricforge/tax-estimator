use std::ops::Deref;
use std::sync::OnceLock;

use gpui::Hsla;

/// An index into the global theme color array.
///
/// This type is `Copy`, allowing it to be used like a constant while
/// the actual color value is initialized at runtime from the system theme.
/// Implements `From<ThemeColor>` for both `Hsla` and `gpui::Fill`, so it
/// works directly with GPUI styling methods like `.bg()`, `.text_color()`, etc.
#[derive(Copy, Clone)]
pub struct ThemeColor(usize);

// Storage for dynamically-initialized theme colors.
static THEME_COLORS: [OnceLock<Hsla>; 4] = [
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
];

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
        *THEME_COLORS[self.0]
            .get()
            .expect("theme colors not initialized; call init_theme_colors() in setup_app()")
    }
}

impl Deref for ThemeColor {
    type Target = Hsla;

    fn deref(&self) -> &Self::Target {
        THEME_COLORS[self.0]
            .get()
            .expect("theme colors not initialized; call init_theme_colors() in setup_app()")
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

/// Initializes the theme color constants from the current system theme.
///
/// Call this once during application startup, **after** `gpui_component::init(cx)`
/// and after applying the platform theme (e.g., `apply_windows_system_theme`).
pub fn init_theme_colors(cx: &gpui::App) {
    use gpui_component::Theme;

    let colors = &Theme::global(cx).colors;

    let _ = THEME_COLORS[IDX_DISPLAY_FIELD_BG].set(colors.muted);
    let _ = THEME_COLORS[IDX_DISPLAY_FIELD_BORDER].set(colors.border);
    let _ = THEME_COLORS[IDX_DISPLAY_FIELD_TEXT].set(colors.muted_foreground);
    let _ = THEME_COLORS[IDX_HEADER_ACCENT].set(colors.primary);
}
