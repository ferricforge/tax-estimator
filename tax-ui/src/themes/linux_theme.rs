use gpui::{App, Hsla};
use tracing::debug;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

use super::{SystemPalette, apply_palette};

// ── Portal constants ──────────────────────────────────────────────

const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Settings";

// ── Local colour types ────────────────────────────────────────────

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

/// Converts a local [`Rgba`] to [`Hsla`] via the shared helper.
fn to_hsla(color: Rgba) -> Hsla {
    super::rgba_to_hsla(color.r, color.g, color.b, color.a)
}

/// Returns a dark or light text colour that is legible on `background`.
fn text_on(background: Rgba) -> Rgba {
    if to_hsla(background).l > 0.55 {
        rgba(0.08, 0.08, 0.08, 1.0)
    } else {
        rgba(0.97, 0.97, 0.97, 1.0)
    }
}

// ── Intermediate Linux palette ────────────────────────────────────

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

impl LinuxPalette {
    /// Converts this platform-specific palette into the shared
    /// [`SystemPalette`] consumed by [`apply_palette`].
    fn into_system_palette(self) -> SystemPalette {
        SystemPalette {
            accent: to_hsla(self.accent),
            accent_foreground: to_hsla(self.alt_selected_text),
            window_bg: to_hsla(self.window_bg),
            control_bg: to_hsla(self.control_bg),
            label: to_hsla(self.label),
            secondary_label: to_hsla(self.secondary_label),
            tertiary_label: to_hsla(self.tertiary_label),
            separator: to_hsla(self.separator),
            selected_text_bg: to_hsla(self.selected_text_bg),
            keyboard_focus: to_hsla(self.keyboard_focus),
            link: to_hsla(self.link),
            unemphasized_bg: to_hsla(self.unemphasized_bg),
            red: to_hsla(self.sys_red),
            orange: to_hsla(self.sys_orange),
            yellow: to_hsla(self.sys_yellow),
            green: to_hsla(self.sys_green),
            teal: to_hsla(self.sys_teal),
            blue: to_hsla(self.sys_blue),
            purple: to_hsla(self.sys_purple),
            pink: to_hsla(self.sys_pink),
        }
    }
}

// ── Portal readers ────────────────────────────────────────────────

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

// ── Palette builder ───────────────────────────────────────────────

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

// ── Public entry point ────────────────────────────────────────────

/// Applies Linux desktop appearance colors to the gpui-component global theme.
///
/// The implementation reads appearance values from the local XDG Desktop Portal
/// settings interface (session D-Bus). If portal values are unavailable, it
/// falls back to a deterministic palette with dark-mode detection from `GTK_THEME`.
pub fn apply_linux_system_theme(cx: &mut App) {
    let dark = prefers_dark();
    let accent = portal_accent_color();

    if accent.is_none() {
        debug!("Portal accent color unavailable; using fallback accent");
    }

    let palette = build_palette(dark, accent).into_system_palette();
    apply_palette(cx, &palette);
}
