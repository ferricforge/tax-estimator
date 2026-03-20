#[cfg(target_os = "linux")]
mod linux_theme;
#[cfg(target_os = "macos")]
mod macos_theme;
#[cfg(target_os = "windows")]
mod windows_theme;

#[cfg(target_os = "linux")]
pub use linux_theme::apply_linux_system_theme;
#[cfg(target_os = "macos")]
pub use macos_theme::apply_macos_system_theme;
#[cfg(target_os = "windows")]
pub use windows_theme::apply_windows_system_theme;

use gpui::{App, Hsla};
use gpui_component::Theme;

// ── Colour-space helpers ──────────────────────────────────────────

/// Converts linear RGB + alpha (each 0.0–1.0) to `Hsla`.
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

// ── Colour derivation helpers ─────────────────────────────────────

/// Derives a lighter variant by raising lightness.
pub fn lighter(base: Hsla) -> Hsla {
    Hsla {
        l: (base.l + 0.15).min(1.0),
        ..base
    }
}

/// Derives a hover variant by shifting lightness toward 50 %.
pub fn hover_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.05 } else { 0.05 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

/// Derives an active / pressed variant by shifting lightness further.
pub fn active_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.10 } else { 0.10 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

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

    // ── Primary ───────────────────────────────────────────────
    colors.primary = palette.accent;
    colors.primary_foreground = palette.accent_foreground;
    colors.primary_hover = hover_variant(palette.accent);
    colors.primary_active = active_variant(palette.accent);

    // ── Accent (hover highlights on menu / list items) ────────
    colors.accent = palette.unemphasized_bg;
    colors.accent_foreground = palette.label;

    // ── Background / foreground ───────────────────────────────
    colors.background = palette.window_bg;
    colors.foreground = palette.label;

    // ── Secondary ─────────────────────────────────────────────
    colors.secondary = palette.unemphasized_bg;
    colors.secondary_foreground = palette.secondary_label;
    colors.secondary_hover = hover_variant(palette.unemphasized_bg);
    colors.secondary_active = active_variant(palette.unemphasized_bg);

    // ── Muted ─────────────────────────────────────────────────
    colors.muted = palette.unemphasized_bg;
    colors.muted_foreground = palette.secondary_label;

    // ── Popover ───────────────────────────────────────────────
    colors.popover = palette.control_bg;
    colors.popover_foreground = palette.label;

    // ── Borders / input / ring ────────────────────────────────
    colors.border = palette.separator;
    colors.input = palette.separator;
    colors.ring = palette.keyboard_focus;

    // ── Selection / caret ─────────────────────────────────────
    colors.selection = palette.selected_text_bg;
    colors.caret = palette.accent;

    // ── Link ──────────────────────────────────────────────────
    colors.link = palette.link;
    colors.link_hover = hover_variant(palette.link);
    colors.link_active = active_variant(palette.link);

    // ── Danger (red) ──────────────────────────────────────────
    colors.danger = palette.red;
    colors.danger_foreground = palette.accent_foreground;
    colors.danger_hover = hover_variant(palette.red);
    colors.danger_active = active_variant(palette.red);

    // ── Success (green) ───────────────────────────────────────
    colors.success = palette.green;
    colors.success_foreground = palette.accent_foreground;
    colors.success_hover = hover_variant(palette.green);
    colors.success_active = active_variant(palette.green);

    // ── Warning (orange) ──────────────────────────────────────
    colors.warning = palette.orange;
    colors.warning_foreground = palette.accent_foreground;
    colors.warning_hover = hover_variant(palette.orange);
    colors.warning_active = active_variant(palette.orange);

    // ── Info (blue) ───────────────────────────────────────────
    colors.info = palette.blue;
    colors.info_foreground = palette.accent_foreground;
    colors.info_hover = hover_variant(palette.blue);
    colors.info_active = active_variant(palette.blue);

    // ── Named palette colours ─────────────────────────────────
    colors.red = palette.red;
    colors.red_light = lighter(palette.red);
    colors.green = palette.green;
    colors.green_light = lighter(palette.green);
    colors.blue = palette.blue;
    colors.blue_light = lighter(palette.blue);
    colors.yellow = palette.yellow;
    colors.yellow_light = lighter(palette.yellow);
    colors.cyan = palette.teal;
    colors.cyan_light = lighter(palette.teal);
    colors.magenta = palette.pink;
    colors.magenta_light = lighter(palette.pink);

    // ── Candlestick chart ─────────────────────────────────────
    colors.bullish = palette.green;
    colors.bearish = palette.red;

    // ── Sidebar ───────────────────────────────────────────────
    colors.sidebar = palette.window_bg;
    colors.sidebar_foreground = palette.label;
    colors.sidebar_border = palette.separator;
    colors.sidebar_accent = palette.unemphasized_bg;
    colors.sidebar_accent_foreground = palette.label;
    colors.sidebar_primary = palette.accent;
    colors.sidebar_primary_foreground = palette.accent_foreground;

    // ── Title bar ─────────────────────────────────────────────
    colors.title_bar = palette.window_bg;
    colors.title_bar_border = palette.separator;

    // ── Window border ─────────────────────────────────────────
    colors.window_border = palette.separator;

    // ── Overlay (uniform across platforms) ─────────────────────
    colors.overlay = rgba_to_hsla(0.0, 0.0, 0.0, 0.4);

    // ── List ──────────────────────────────────────────────────
    colors.list = palette.control_bg;
    colors.list_hover = palette.unemphasized_bg;
    colors.list_active = palette.accent;
    colors.list_active_border = palette.accent;
    colors.list_head = palette.window_bg;
    colors.list_even = hover_variant(palette.control_bg);

    // ── Table ─────────────────────────────────────────────────
    colors.table = palette.control_bg;
    colors.table_hover = palette.unemphasized_bg;
    colors.table_active = palette.accent;
    colors.table_active_border = palette.accent;
    colors.table_head = palette.window_bg;
    colors.table_head_foreground = palette.label;
    colors.table_row_border = palette.separator;
    colors.table_even = hover_variant(palette.control_bg);

    // ── Tab ───────────────────────────────────────────────────
    colors.tab = palette.window_bg;
    colors.tab_foreground = palette.secondary_label;
    colors.tab_active = palette.control_bg;
    colors.tab_active_foreground = palette.label;
    colors.tab_bar = palette.window_bg;
    colors.tab_bar_segmented = palette.unemphasized_bg;

    // ── Scrollbar ─────────────────────────────────────────────
    colors.scrollbar = palette.window_bg;
    colors.scrollbar_thumb = palette.tertiary_label;
    colors.scrollbar_thumb_hover = palette.secondary_label;

    // ── Slider ────────────────────────────────────────────────
    colors.slider_bar = palette.unemphasized_bg;
    colors.slider_thumb = palette.control_bg;

    // ── Switch ────────────────────────────────────────────────
    colors.switch = palette.unemphasized_bg;
    colors.switch_thumb = palette.control_bg;

    // ── Progress bar ──────────────────────────────────────────
    colors.progress_bar = palette.accent;

    // ── Skeleton ──────────────────────────────────────────────
    colors.skeleton = palette.unemphasized_bg;

    // ── Accordion ─────────────────────────────────────────────
    colors.accordion = palette.control_bg;
    colors.accordion_hover = palette.unemphasized_bg;

    // ── GroupBox ───────────────────────────────────────────────
    colors.group_box = palette.control_bg;
    colors.group_box_foreground = palette.label;

    // ── DescriptionList ───────────────────────────────────────
    colors.description_list_label = palette.unemphasized_bg;
    colors.description_list_label_foreground = palette.secondary_label;

    // ── Drag / drop ───────────────────────────────────────────
    colors.drag_border = palette.accent;
    colors.drop_target = palette.unemphasized_bg;

    // ── Tiles ─────────────────────────────────────────────────
    colors.tiles = palette.control_bg;

    // ── Chart palette ─────────────────────────────────────────
    colors.chart_1 = palette.blue;
    colors.chart_2 = palette.green;
    colors.chart_3 = palette.orange;
    colors.chart_4 = palette.purple;
    colors.chart_5 = palette.teal;
}
