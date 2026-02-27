use gpui::{App, Hsla};
use gpui_component::Theme;
use tracing::debug;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Settings";

#[derive(Clone, Copy, Debug)]
enum ColorScheme {
    NoPreference,
    PreferDark,
    PreferLight,
}

#[derive(Clone, Copy, Debug)]
struct Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Clone, Copy, Debug)]
struct LinuxPalette {
    accent: Rgba,
    window_bg: Rgba,
    control_bg: Rgba,
    label: Rgba,
    secondary_label: Rgba,
    tertiary_label: Rgba,
    separator: Rgba,
    selected_text_bg: Rgba,
    keyboard_focus: Rgba,
    link: Rgba,
    alt_selected_text: Rgba,
    unemphasized_bg: Rgba,
    sys_red: Rgba,
    sys_orange: Rgba,
    sys_yellow: Rgba,
    sys_green: Rgba,
    sys_teal: Rgba,
    sys_blue: Rgba,
    sys_purple: Rgba,
    sys_pink: Rgba,
}

fn rgba(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) -> Rgba {
    Rgba { r, g, b, a }
}

fn with_alpha(
    color: Rgba,
    a: f32,
) -> Rgba {
    Rgba { a, ..color }
}

fn normalize_channel(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn rgba_to_hsla(color: Rgba) -> Hsla {
    let max = color.r.max(color.g).max(color.b);
    let min = color.r.min(color.g).min(color.b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a: color.a,
        };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - color.r).abs() < f32::EPSILON {
        ((color.g - color.b) / d + if color.g < color.b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - color.g).abs() < f32::EPSILON {
        ((color.b - color.r) / d + 2.0) / 6.0
    } else {
        ((color.r - color.g) / d + 4.0) / 6.0
    };

    Hsla {
        h,
        s,
        l,
        a: color.a,
    }
}

fn apply(
    target: &mut Hsla,
    color: Rgba,
) {
    *target = rgba_to_hsla(color);
}

fn lighter(base: Hsla) -> Hsla {
    Hsla {
        l: (base.l + 0.15).min(1.0),
        ..base
    }
}

fn hover_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.05 } else { 0.05 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

fn active_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.10 } else { 0.10 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

fn text_on(background: Rgba) -> Rgba {
    if rgba_to_hsla(background).l > 0.55 {
        rgba(0.08, 0.08, 0.08, 1.0)
    } else {
        rgba(0.97, 0.97, 0.97, 1.0)
    }
}

fn read_portal_setting(
    namespace: &str,
    key: &str,
) -> Option<OwnedValue> {
    let connection = Connection::session().ok()?;
    let proxy = Proxy::new(&connection, PORTAL_SERVICE, PORTAL_PATH, PORTAL_INTERFACE).ok()?;

    proxy.call("ReadOne", &(namespace, key)).ok()
}

fn parse_color_scheme(value: OwnedValue) -> Option<ColorScheme> {
    let raw = value
        .try_clone()
        .ok()
        .and_then(|v| u32::try_from(v).ok())
        .or_else(|| {
            value
                .try_clone()
                .ok()
                .and_then(|v| i32::try_from(v).ok())
                .and_then(|v| u32::try_from(v).ok())
        })?;

    match raw {
        0 => Some(ColorScheme::NoPreference),
        1 => Some(ColorScheme::PreferDark),
        2 => Some(ColorScheme::PreferLight),
        _ => None,
    }
}

fn parse_accent_color(value: OwnedValue) -> Option<Rgba> {
    if let Ok(v) = value.try_clone() {
        if let Ok((r, g, b)) = <(f64, f64, f64)>::try_from(v) {
            return Some(rgba(
                normalize_channel(r as f32),
                normalize_channel(g as f32),
                normalize_channel(b as f32),
                1.0,
            ));
        }
    }

    if let Ok(v) = value.try_clone() {
        if let Ok((r, g, b, a)) = <(f64, f64, f64, f64)>::try_from(v) {
            return Some(rgba(
                normalize_channel(r as f32),
                normalize_channel(g as f32),
                normalize_channel(b as f32),
                normalize_channel(a as f32),
            ));
        }
    }

    if let Ok(components) = Vec::<f64>::try_from(value) {
        if components.len() >= 3 {
            return Some(rgba(
                normalize_channel(components[0] as f32),
                normalize_channel(components[1] as f32),
                normalize_channel(components[2] as f32),
                1.0,
            ));
        }
    }

    None
}

fn portal_color_scheme() -> Option<ColorScheme> {
    read_portal_setting("org.freedesktop.appearance", "color-scheme").and_then(parse_color_scheme)
}

fn portal_accent_color() -> Option<Rgba> {
    read_portal_setting("org.freedesktop.appearance", "accent-color").and_then(parse_accent_color)
}

fn prefers_dark() -> bool {
    match portal_color_scheme() {
        Some(ColorScheme::PreferDark) => true,
        Some(ColorScheme::PreferLight) => false,
        Some(ColorScheme::NoPreference) | None => std::env::var("GTK_THEME")
            .map(|theme| theme.to_ascii_lowercase().contains("dark"))
            .unwrap_or(false),
    }
}

fn build_palette(
    dark: bool,
    accent: Option<Rgba>,
) -> LinuxPalette {
    let default_accent = if dark {
        rgba(0.45, 0.64, 1.0, 1.0)
    } else {
        rgba(0.16, 0.36, 0.95, 1.0)
    };
    let accent = accent.unwrap_or(default_accent);
    let alt_selected_text = text_on(accent);

    if dark {
        LinuxPalette {
            accent,
            window_bg: rgba(0.10, 0.11, 0.12, 1.0),
            control_bg: rgba(0.16, 0.17, 0.19, 1.0),
            label: rgba(0.92, 0.93, 0.95, 1.0),
            secondary_label: rgba(0.74, 0.76, 0.79, 1.0),
            tertiary_label: rgba(0.58, 0.60, 0.64, 1.0),
            separator: rgba(0.28, 0.30, 0.33, 1.0),
            selected_text_bg: with_alpha(accent, 0.85),
            keyboard_focus: accent,
            link: accent,
            alt_selected_text,
            unemphasized_bg: rgba(0.23, 0.24, 0.27, 1.0),
            sys_red: rgba(0.93, 0.33, 0.32, 1.0),
            sys_orange: rgba(0.95, 0.59, 0.24, 1.0),
            sys_yellow: rgba(0.94, 0.79, 0.30, 1.0),
            sys_green: rgba(0.34, 0.78, 0.45, 1.0),
            sys_teal: rgba(0.31, 0.76, 0.73, 1.0),
            sys_blue: rgba(0.40, 0.68, 1.0, 1.0),
            sys_purple: rgba(0.72, 0.52, 0.98, 1.0),
            sys_pink: rgba(0.95, 0.47, 0.76, 1.0),
        }
    } else {
        LinuxPalette {
            accent,
            window_bg: rgba(0.97, 0.97, 0.98, 1.0),
            control_bg: rgba(1.0, 1.0, 1.0, 1.0),
            label: rgba(0.13, 0.13, 0.14, 1.0),
            secondary_label: rgba(0.32, 0.33, 0.35, 1.0),
            tertiary_label: rgba(0.47, 0.48, 0.51, 1.0),
            separator: rgba(0.81, 0.82, 0.84, 1.0),
            selected_text_bg: with_alpha(accent, 0.90),
            keyboard_focus: accent,
            link: accent,
            alt_selected_text,
            unemphasized_bg: rgba(0.92, 0.93, 0.95, 1.0),
            sys_red: rgba(0.86, 0.25, 0.24, 1.0),
            sys_orange: rgba(0.91, 0.49, 0.10, 1.0),
            sys_yellow: rgba(0.85, 0.67, 0.18, 1.0),
            sys_green: rgba(0.17, 0.66, 0.30, 1.0),
            sys_teal: rgba(0.14, 0.63, 0.60, 1.0),
            sys_blue: rgba(0.20, 0.46, 0.97, 1.0),
            sys_purple: rgba(0.56, 0.35, 0.88, 1.0),
            sys_pink: rgba(0.84, 0.27, 0.57, 1.0),
        }
    }
}

/// Applies Linux desktop appearance colors to the gpui-component global theme.
///
/// The implementation reads appearance values from the local XDG Desktop Portal
/// settings interface (session D-Bus). If portal values are unavailable, it
/// falls back to a deterministic palette with dark-mode detection from GTK_THEME.
pub fn apply_linux_system_theme(cx: &mut App) {
    let dark = prefers_dark();
    let accent = portal_accent_color();

    if accent.is_none() {
        debug!("Portal accent color unavailable; using fallback accent");
    }

    let palette = build_palette(dark, accent);
    let colors = &mut Theme::global_mut(cx).colors;

    // ── Primary ───────────────────────────────────────────────────
    apply(&mut colors.primary, palette.accent);
    apply(&mut colors.primary_foreground, palette.alt_selected_text);
    let accent_hsla = rgba_to_hsla(palette.accent);
    colors.primary_hover = hover_variant(accent_hsla);
    colors.primary_active = active_variant(accent_hsla);

    // ── Accent / background / foreground ──────────────────────────
    apply(&mut colors.accent, palette.unemphasized_bg);
    apply(&mut colors.accent_foreground, palette.label);
    apply(&mut colors.background, palette.window_bg);
    apply(&mut colors.foreground, palette.label);

    // ── Secondary / muted / popover ───────────────────────────────
    apply(&mut colors.secondary, palette.unemphasized_bg);
    apply(&mut colors.secondary_foreground, palette.secondary_label);
    let unemphasized_hsla = rgba_to_hsla(palette.unemphasized_bg);
    colors.secondary_hover = hover_variant(unemphasized_hsla);
    colors.secondary_active = active_variant(unemphasized_hsla);
    apply(&mut colors.muted, palette.unemphasized_bg);
    apply(&mut colors.muted_foreground, palette.secondary_label);
    apply(&mut colors.popover, palette.control_bg);
    apply(&mut colors.popover_foreground, palette.label);

    // ── Borders / selection / ring / link ────────────────────────
    apply(&mut colors.border, palette.separator);
    apply(&mut colors.input, palette.separator);
    apply(&mut colors.ring, palette.keyboard_focus);
    apply(&mut colors.selection, palette.selected_text_bg);
    apply(&mut colors.caret, palette.accent);
    apply(&mut colors.link, palette.link);
    let link_hsla = rgba_to_hsla(palette.link);
    colors.link_hover = hover_variant(link_hsla);
    colors.link_active = active_variant(link_hsla);

    // ── Semantic colors ───────────────────────────────────────────
    apply(&mut colors.danger, palette.sys_red);
    apply(&mut colors.danger_foreground, palette.alt_selected_text);
    let danger_hsla = rgba_to_hsla(palette.sys_red);
    colors.danger_hover = hover_variant(danger_hsla);
    colors.danger_active = active_variant(danger_hsla);

    apply(&mut colors.success, palette.sys_green);
    apply(&mut colors.success_foreground, palette.alt_selected_text);
    let success_hsla = rgba_to_hsla(palette.sys_green);
    colors.success_hover = hover_variant(success_hsla);
    colors.success_active = active_variant(success_hsla);

    apply(&mut colors.warning, palette.sys_orange);
    apply(&mut colors.warning_foreground, palette.alt_selected_text);
    let warning_hsla = rgba_to_hsla(palette.sys_orange);
    colors.warning_hover = hover_variant(warning_hsla);
    colors.warning_active = active_variant(warning_hsla);

    apply(&mut colors.info, palette.sys_blue);
    apply(&mut colors.info_foreground, palette.alt_selected_text);
    let info_hsla = rgba_to_hsla(palette.sys_blue);
    colors.info_hover = hover_variant(info_hsla);
    colors.info_active = active_variant(info_hsla);

    // ── Named palette ─────────────────────────────────────────────
    apply(&mut colors.red, palette.sys_red);
    apply(&mut colors.green, palette.sys_green);
    apply(&mut colors.blue, palette.sys_blue);
    apply(&mut colors.yellow, palette.sys_yellow);
    apply(&mut colors.cyan, palette.sys_teal);
    apply(&mut colors.magenta, palette.sys_pink);

    colors.red_light = lighter(rgba_to_hsla(palette.sys_red));
    colors.green_light = lighter(rgba_to_hsla(palette.sys_green));
    colors.blue_light = lighter(rgba_to_hsla(palette.sys_blue));
    colors.yellow_light = lighter(rgba_to_hsla(palette.sys_yellow));
    colors.cyan_light = lighter(rgba_to_hsla(palette.sys_teal));
    colors.magenta_light = lighter(rgba_to_hsla(palette.sys_pink));

    // ── Candlestick chart ─────────────────────────────────────────
    apply(&mut colors.bullish, palette.sys_green);
    apply(&mut colors.bearish, palette.sys_red);

    // ── Sidebar / title bar / window border ──────────────────────
    apply(&mut colors.sidebar, palette.window_bg);
    apply(&mut colors.sidebar_foreground, palette.label);
    apply(&mut colors.sidebar_border, palette.separator);
    apply(&mut colors.sidebar_accent, palette.unemphasized_bg);
    apply(&mut colors.sidebar_accent_foreground, palette.label);
    apply(&mut colors.sidebar_primary, palette.accent);
    apply(
        &mut colors.sidebar_primary_foreground,
        palette.alt_selected_text,
    );

    apply(&mut colors.title_bar, palette.window_bg);
    apply(&mut colors.title_bar_border, palette.separator);
    apply(&mut colors.window_border, palette.separator);

    // ── Overlay ───────────────────────────────────────────────────
    apply(&mut colors.overlay, rgba(0.0, 0.0, 0.0, 0.4));

    // ── List / table ──────────────────────────────────────────────
    apply(&mut colors.list, palette.control_bg);
    apply(&mut colors.list_hover, palette.unemphasized_bg);
    apply(&mut colors.list_active, palette.accent);
    apply(&mut colors.list_active_border, palette.accent);
    apply(&mut colors.list_head, palette.window_bg);
    colors.list_even = hover_variant(rgba_to_hsla(palette.control_bg));

    apply(&mut colors.table, palette.control_bg);
    apply(&mut colors.table_hover, palette.unemphasized_bg);
    apply(&mut colors.table_active, palette.accent);
    apply(&mut colors.table_active_border, palette.accent);
    apply(&mut colors.table_head, palette.window_bg);
    apply(&mut colors.table_head_foreground, palette.label);
    apply(&mut colors.table_row_border, palette.separator);
    colors.table_even = hover_variant(rgba_to_hsla(palette.control_bg));

    // ── Tabs / scrollbars / sliders / switches ───────────────────
    apply(&mut colors.tab, palette.window_bg);
    apply(&mut colors.tab_foreground, palette.secondary_label);
    apply(&mut colors.tab_active, palette.control_bg);
    apply(&mut colors.tab_active_foreground, palette.label);
    apply(&mut colors.tab_bar, palette.window_bg);
    apply(&mut colors.tab_bar_segmented, palette.unemphasized_bg);

    apply(&mut colors.scrollbar, palette.window_bg);
    apply(&mut colors.scrollbar_thumb, palette.tertiary_label);
    apply(&mut colors.scrollbar_thumb_hover, palette.secondary_label);

    apply(&mut colors.slider_bar, palette.unemphasized_bg);
    apply(&mut colors.slider_thumb, palette.control_bg);

    apply(&mut colors.switch, palette.unemphasized_bg);
    apply(&mut colors.switch_thumb, palette.control_bg);

    // ── Remaining surface colors ──────────────────────────────────
    apply(&mut colors.progress_bar, palette.accent);
    apply(&mut colors.skeleton, palette.unemphasized_bg);
    apply(&mut colors.accordion, palette.control_bg);
    apply(&mut colors.accordion_hover, palette.unemphasized_bg);
    apply(&mut colors.group_box, palette.control_bg);
    apply(&mut colors.group_box_foreground, palette.label);
    apply(&mut colors.description_list_label, palette.unemphasized_bg);
    apply(
        &mut colors.description_list_label_foreground,
        palette.secondary_label,
    );
    apply(&mut colors.drag_border, palette.accent);
    apply(&mut colors.drop_target, palette.unemphasized_bg);
    apply(&mut colors.tiles, palette.control_bg);

    // ── Chart palette ─────────────────────────────────────────────
    apply(&mut colors.chart_1, palette.sys_blue);
    apply(&mut colors.chart_2, palette.sys_green);
    apply(&mut colors.chart_3, palette.sys_orange);
    apply(&mut colors.chart_4, palette.sys_purple);
    apply(&mut colors.chart_5, palette.sys_teal);
}
