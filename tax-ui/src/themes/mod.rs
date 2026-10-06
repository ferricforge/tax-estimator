#[cfg(target_os = "linux")]
mod linux_theme;
#[cfg(target_os = "macos")]
mod macos_theme;
#[cfg(target_os = "windows")]
mod windows_theme;

#[cfg(target_os = "linux")]
pub use linux_theme::apply_linux_system_theme as apply_theme;
#[cfg(target_os = "macos")]
pub use macos_theme::apply_macos_system_theme as apply_theme;
#[cfg(target_os = "windows")]
pub use windows_theme::apply_windows_system_theme as apply_theme;

use gpui::{App, Hsla};
use gpui_component::Theme;
use serde::{Deserialize, Serialize};

// ── Colour-space helpers ──────────────────────────────────────────

/// Converts sRGB components and alpha (each 0.0–1.0) to `Hsla`.
pub fn rgba_to_hsla(
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
) -> Hsla {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l: lightness,
            a: alpha,
        };
    }

    let delta = max - min;
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if (max - red).abs() < f32::EPSILON {
        (green - blue) / delta + if green < blue { 6.0 } else { 0.0 }
    } else if (max - green).abs() < f32::EPSILON {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };

    Hsla {
        h: hue / 6.0,
        s: saturation,
        l: lightness,
        a: alpha,
    }
}

/// Creates an opaque `Hsla` from a 24-bit hex colour (`0xRRGGBB`).
pub fn hex(rgb: u32) -> Hsla {
    let red = ((rgb >> 16) & 0xFF) as f32 / 255.0;
    let green = ((rgb >> 8) & 0xFF) as f32 / 255.0;
    let blue = (rgb & 0xFF) as f32 / 255.0;
    rgba_to_hsla(red, green, blue, 1.0)
}

/// Chooses between a light-mode and a dark-mode value.
///
/// Platform backends describe their surfaces as light/dark pairs, so the
/// mode test stays on one line per role instead of duplicating a whole
/// palette per appearance.
pub fn pick<T>(
    dark: bool,
    light_value: T,
    dark_value: T,
) -> T {
    if dark { dark_value } else { light_value }
}

// ── Colour derivation helpers ─────────────────────────────────────

/// Shifts lightness by `delta`, keeping the result within 0.0–1.0.
fn shift_lightness(
    base: Hsla,
    delta: f32,
) -> Hsla {
    Hsla {
        l: (base.l + delta).clamp(0.0, 1.0),
        ..base
    }
}

/// Derives a lighter variant by raising lightness.
pub fn lighter(base: Hsla) -> Hsla {
    shift_lightness(base, 0.15)
}

/// Derives a hover variant by shifting lightness toward 50 %.
pub fn hover_variant(base: Hsla) -> Hsla {
    shift_lightness(base, if base.l > 0.5 { -0.05 } else { 0.05 })
}

/// Derives an active / pressed variant by shifting lightness further.
pub fn active_variant(base: Hsla) -> Hsla {
    shift_lightness(base, if base.l > 0.5 { -0.10 } else { 0.10 })
}

/// Returns `base` together with its hover and active variants.
fn interactive(base: Hsla) -> (Hsla, Hsla, Hsla) {
    (base, hover_variant(base), active_variant(base))
}

// ── Appearance mode ───────────────────────────────────────────────

/// The appearance the user chose. `System` follows the operating system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    /// Every mode, in the order the preferences control steps through them.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    /// Whether this mode is dark. `system_is_dark` is called only for
    /// `System`, so forced modes never query the operating system.
    pub fn is_dark(
        self,
        system_is_dark: impl FnOnce() -> bool,
    ) -> bool {
        match self {
            Self::System => system_is_dark(),
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// Installs `mode` with the window system before the palette is read.
///
/// Call this while no `App` borrow is held. AppKit notifies the open windows
/// during the call, and gpui borrows the application to handle that.
#[cfg(target_os = "macos")]
pub fn install_appearance(mode: ThemeMode) {
    macos_theme::install_appearance(mode);
}

/// Installs `mode` with the window system before the palette is read.
///
/// Linux and Windows choose their palette from `mode` directly, so there is
/// nothing to install.
#[cfg(not(target_os = "macos"))]
pub fn install_appearance(_mode: ThemeMode) {}

// ── Platform-neutral semantic palette ─────────────────────────────

/// Every semantic colour slot that the platform backends must fill.
///
/// Each field corresponds to a role the OS theme can provide (accent,
/// window background, label text, etc.).  Platform modules construct
/// this struct from native APIs; the shared [`apply_palette`] function
/// maps it onto every [`gpui_component::ThemeColor`] field.
pub struct SystemPalette {
    pub accent: Hsla,
    pub accent_foreground: Hsla,
    pub window_bg: Hsla,
    pub control_bg: Hsla,
    pub label: Hsla,
    pub secondary_label: Hsla,
    pub tertiary_label: Hsla,
    pub separator: Hsla,
    pub selected_text_bg: Hsla,
    pub keyboard_focus: Hsla,
    pub link: Hsla,
    pub unemphasized_bg: Hsla,

    // Semantic colours
    pub red: Hsla,
    pub orange: Hsla,
    pub yellow: Hsla,
    pub green: Hsla,
    pub teal: Hsla,
    pub blue: Hsla,
    pub purple: Hsla,
    pub pink: Hsla,
}

// ── Shared applicator ─────────────────────────────────────────────

/// Maps a [`SystemPalette`] onto the global `Theme` colours.
///
/// Call this from each platform's `apply_*_system_theme` function
/// after constructing the palette from native APIs.
pub fn apply_palette(
    cx: &mut App,
    palette: &SystemPalette,
) {
    let colors = &mut Theme::global_mut(cx).colors;

    // Interactive roles: base colour plus its hover and active variants.
    (colors.primary, colors.primary_hover, colors.primary_active) = interactive(palette.accent);
    colors.primary_foreground = palette.accent_foreground;

    (
        colors.secondary,
        colors.secondary_hover,
        colors.secondary_active,
    ) = interactive(palette.unemphasized_bg);
    colors.secondary_foreground = palette.secondary_label;

    (colors.link, colors.link_hover, colors.link_active) = interactive(palette.link);

    (colors.danger, colors.danger_hover, colors.danger_active) = interactive(palette.red);
    colors.danger_foreground = palette.accent_foreground;

    (colors.success, colors.success_hover, colors.success_active) = interactive(palette.green);
    colors.success_foreground = palette.accent_foreground;

    (colors.warning, colors.warning_hover, colors.warning_active) = interactive(palette.orange);
    colors.warning_foreground = palette.accent_foreground;

    (colors.info, colors.info_hover, colors.info_active) = interactive(palette.blue);
    colors.info_foreground = palette.accent_foreground;

    // Surfaces and text. `accent` here is the hover highlight used by
    // menu and list items, not the operating system accent colour.
    colors.background = palette.window_bg;
    colors.foreground = palette.label;
    colors.accent = palette.unemphasized_bg;
    colors.accent_foreground = palette.label;
    colors.muted = palette.unemphasized_bg;
    colors.muted_foreground = palette.secondary_label;
    colors.popover = palette.control_bg;
    colors.popover_foreground = palette.label;
    colors.overlay = rgba_to_hsla(0.0, 0.0, 0.0, 0.4);

    // Borders, focus ring, selection, and caret.
    colors.border = palette.separator;
    colors.input = palette.separator;
    colors.ring = palette.keyboard_focus;
    colors.window_border = palette.separator;
    colors.selection = palette.selected_text_bg;
    colors.caret = palette.accent;

    // Named palette colours, each with a lighter companion.
    (colors.red, colors.red_light) = (palette.red, lighter(palette.red));
    (colors.green, colors.green_light) = (palette.green, lighter(palette.green));
    (colors.blue, colors.blue_light) = (palette.blue, lighter(palette.blue));
    (colors.yellow, colors.yellow_light) = (palette.yellow, lighter(palette.yellow));
    (colors.cyan, colors.cyan_light) = (palette.teal, lighter(palette.teal));
    (colors.magenta, colors.magenta_light) = (palette.pink, lighter(palette.pink));

    // Candlestick chart.
    colors.bullish = palette.green;
    colors.bearish = palette.red;

    // Sidebar.
    colors.sidebar = palette.window_bg;
    colors.sidebar_foreground = palette.label;
    colors.sidebar_border = palette.separator;
    colors.sidebar_accent = palette.unemphasized_bg;
    colors.sidebar_accent_foreground = palette.label;
    colors.sidebar_primary = palette.accent;
    colors.sidebar_primary_foreground = palette.accent_foreground;

    // Title bar.
    colors.title_bar = palette.window_bg;
    colors.title_bar_border = palette.separator;

    // List.
    colors.list = palette.control_bg;
    colors.list_hover = palette.unemphasized_bg;
    colors.list_active = palette.accent;
    colors.list_active_border = palette.accent;
    colors.list_head = palette.window_bg;
    colors.list_even = hover_variant(palette.control_bg);

    // Table.
    colors.table = palette.control_bg;
    colors.table_hover = palette.unemphasized_bg;
    colors.table_active = palette.accent;
    colors.table_active_border = palette.accent;
    colors.table_head = palette.window_bg;
    colors.table_head_foreground = palette.label;
    colors.table_row_border = palette.separator;
    colors.table_even = hover_variant(palette.control_bg);

    // Tab.
    colors.tab = palette.window_bg;
    colors.tab_foreground = palette.secondary_label;
    colors.tab_active = palette.control_bg;
    colors.tab_active_foreground = palette.label;
    colors.tab_bar = palette.window_bg;
    colors.tab_bar_segmented = palette.unemphasized_bg;

    // Scrollbar, slider, and switch.
    colors.scrollbar = palette.window_bg;
    colors.scrollbar_thumb = palette.tertiary_label;
    colors.scrollbar_thumb_hover = palette.secondary_label;
    colors.slider_bar = palette.unemphasized_bg;
    colors.slider_thumb = palette.control_bg;
    colors.switch = palette.unemphasized_bg;
    colors.switch_thumb = palette.control_bg;

    // Progress bar, skeleton, and accordion.
    colors.progress_bar = palette.accent;
    colors.skeleton = palette.unemphasized_bg;
    colors.accordion = palette.control_bg;
    colors.accordion_hover = palette.unemphasized_bg;

    // GroupBox and DescriptionList.
    colors.group_box = palette.control_bg;
    colors.group_box_foreground = palette.label;
    colors.description_list_label = palette.unemphasized_bg;
    colors.description_list_label_foreground = palette.secondary_label;

    // Drag and drop, tiles.
    colors.drag_border = palette.accent;
    colors.drop_target = palette.unemphasized_bg;
    colors.tiles = palette.control_bg;

    // Chart palette.
    colors.chart_1 = palette.blue;
    colors.chart_2 = palette.green;
    colors.chart_3 = palette.orange;
    colors.chart_4 = palette.purple;
    colors.chart_5 = palette.teal;
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOLERANCE: f32 = 1e-6;

    #[test]
    fn hex_converts_a_saturated_primary() {
        let red = hex(0xFF0000);

        assert!((red.h - 0.0).abs() < TOLERANCE);
        assert!((red.s - 1.0).abs() < TOLERANCE);
        assert!((red.l - 0.5).abs() < TOLERANCE);
        assert!((red.a - 1.0).abs() < TOLERANCE);
    }

    #[test]
    fn grey_has_no_saturation() {
        let grey = hex(0x808080);

        assert!((grey.s - 0.0).abs() < TOLERANCE);
        assert!((grey.h - 0.0).abs() < TOLERANCE);
    }

    #[test]
    fn variants_move_toward_mid_lightness() {
        let dark = Hsla {
            h: 0.6,
            s: 0.5,
            l: 0.2,
            a: 1.0,
        };
        let light = Hsla { l: 0.8, ..dark };

        assert!(hover_variant(dark).l > dark.l);
        assert!(active_variant(dark).l > hover_variant(dark).l);
        assert!(hover_variant(light).l < light.l);
        assert!(active_variant(light).l < hover_variant(light).l);
    }

    #[test]
    fn derived_lightness_stays_in_range() {
        let white = Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 1.0,
        };
        let black = Hsla { l: 0.0, ..white };

        assert!(lighter(white).l <= 1.0);
        assert!(active_variant(black).l >= 0.0);
    }

    #[test]
    fn pick_follows_the_mode_flag() {
        assert_eq!(pick(true, 1_u32, 2_u32), 2);
        assert_eq!(pick(false, 1_u32, 2_u32), 1);
    }

    #[test]
    fn system_is_the_default_mode() {
        assert_eq!(ThemeMode::default(), ThemeMode::System);
    }

    #[test]
    fn forced_modes_ignore_the_system() {
        assert!(ThemeMode::Dark.is_dark(|| false));
        assert!(!ThemeMode::Light.is_dark(|| true));
    }

    #[test]
    fn system_mode_uses_the_system_answer() {
        assert!(ThemeMode::System.is_dark(|| true));
        assert!(!ThemeMode::System.is_dark(|| false));
    }
}
