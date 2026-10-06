use gpui::{App, Hsla};
use tracing::debug;
use zbus::{
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

use super::{SystemPalette, ThemeMode, apply_palette, pick, rgba_to_hsla};

// ── Portal constants ──────────────────────────────────────────────

const PORTAL_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_INTERFACE: &str = "org.freedesktop.portal.Settings";
const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";

// ── Colour helpers ────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
enum ColorScheme {
    NoPreference,
    PreferDark,
    PreferLight,
}

/// Creates an opaque colour from sRGB components (each 0.0–1.0).
fn rgb(
    red: f32,
    green: f32,
    blue: f32,
) -> Hsla {
    rgba_to_hsla(red, green, blue, 1.0)
}

/// Accepts portal channels expressed either as 0.0–1.0 or as 0–255.
fn normalize_channel(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// Returns a dark or light text colour that is legible on `background`.
fn text_on(background: Hsla) -> Hsla {
    if background.l > 0.55 {
        rgb(0.08, 0.08, 0.08)
    } else {
        rgb(0.97, 0.97, 0.97)
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

/// Reads a portal value that may arrive as either `u32` or `i32`.
fn as_u32(value: &OwnedValue) -> Option<u32> {
    value
        .try_clone()
        .ok()
        .and_then(|cloned| u32::try_from(cloned).ok())
        .or_else(|| {
            value
                .try_clone()
                .ok()
                .and_then(|cloned| i32::try_from(cloned).ok())
                .and_then(|signed| u32::try_from(signed).ok())
        })
}

fn parse_color_scheme(value: OwnedValue) -> Option<ColorScheme> {
    match as_u32(&value)? {
        0 => Some(ColorScheme::NoPreference),
        1 => Some(ColorScheme::PreferDark),
        2 => Some(ColorScheme::PreferLight),
        _ => None,
    }
}

/// Parses an accent colour sent as a 3-tuple, a 4-tuple, or a list.
fn parse_accent_color(value: OwnedValue) -> Option<Hsla> {
    let triple = value
        .try_clone()
        .ok()
        .and_then(|cloned| <(f64, f64, f64)>::try_from(cloned).ok())
        .map(|(red, green, blue)| [red, green, blue, 1.0]);

    let quad = value
        .try_clone()
        .ok()
        .and_then(|cloned| <(f64, f64, f64, f64)>::try_from(cloned).ok())
        .map(|(red, green, blue, alpha)| [red, green, blue, alpha]);

    let list = Vec::<f64>::try_from(value)
        .ok()
        .filter(|components| components.len() >= 3)
        .map(|components| [components[0], components[1], components[2], 1.0]);

    let [red, green, blue, alpha] = triple.or(quad).or(list)?;

    Some(rgba_to_hsla(
        normalize_channel(red as f32),
        normalize_channel(green as f32),
        normalize_channel(blue as f32),
        normalize_channel(alpha as f32),
    ))
}

fn portal_color_scheme() -> Option<ColorScheme> {
    read_portal_setting(APPEARANCE_NAMESPACE, "color-scheme").and_then(parse_color_scheme)
}

fn portal_accent_color() -> Option<Hsla> {
    read_portal_setting(APPEARANCE_NAMESPACE, "accent-color").and_then(parse_accent_color)
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

/// Builds the shared palette for the detected appearance.
///
/// Each role lists its light value first and its dark value second, so
/// both appearances stay side by side and cannot drift apart.
fn build_palette(
    dark: bool,
    accent: Option<Hsla>,
) -> SystemPalette {
    let accent = accent.unwrap_or_else(|| pick(dark, rgb(0.16, 0.36, 0.95), rgb(0.45, 0.64, 1.0)));

    SystemPalette {
        accent,
        accent_foreground: text_on(accent),
        window_bg: pick(dark, rgb(0.97, 0.97, 0.98), rgb(0.10, 0.11, 0.12)),
        control_bg: pick(dark, rgb(1.0, 1.0, 1.0), rgb(0.16, 0.17, 0.19)),
        label: pick(dark, rgb(0.13, 0.13, 0.14), rgb(0.92, 0.93, 0.95)),
        secondary_label: pick(dark, rgb(0.32, 0.33, 0.35), rgb(0.74, 0.76, 0.79)),
        tertiary_label: pick(dark, rgb(0.47, 0.48, 0.51), rgb(0.58, 0.60, 0.64)),
        separator: pick(dark, rgb(0.81, 0.82, 0.84), rgb(0.28, 0.30, 0.33)),
        selected_text_bg: Hsla {
            a: pick(dark, 0.90, 0.85),
            ..accent
        },
        keyboard_focus: accent,
        link: accent,
        unemphasized_bg: pick(dark, rgb(0.92, 0.93, 0.95), rgb(0.23, 0.24, 0.27)),
        red: pick(dark, rgb(0.86, 0.25, 0.24), rgb(0.93, 0.33, 0.32)),
        orange: pick(dark, rgb(0.91, 0.49, 0.10), rgb(0.95, 0.59, 0.24)),
        yellow: pick(dark, rgb(0.85, 0.67, 0.18), rgb(0.94, 0.79, 0.30)),
        green: pick(dark, rgb(0.17, 0.66, 0.30), rgb(0.34, 0.78, 0.45)),
        teal: pick(dark, rgb(0.14, 0.63, 0.60), rgb(0.31, 0.76, 0.73)),
        blue: pick(dark, rgb(0.20, 0.46, 0.97), rgb(0.40, 0.68, 1.0)),
        purple: pick(dark, rgb(0.56, 0.35, 0.88), rgb(0.72, 0.52, 0.98)),
        pink: pick(dark, rgb(0.84, 0.27, 0.57), rgb(0.95, 0.47, 0.76)),
    }
}

// ── Public entry point ────────────────────────────────────────────

/// Applies Linux desktop appearance colors to the gpui-component global theme.
///
/// `mode` decides between light and dark. `System` reads the XDG Desktop
/// Portal color scheme (session D-Bus) and falls back to `GTK_THEME`. The
/// accent color comes from the portal when it is available.
pub fn apply_linux_system_theme(
    cx: &mut App,
    mode: ThemeMode,
) {
    let dark = mode.is_dark(prefers_dark);
    let accent = portal_accent_color();

    if accent.is_none() {
        debug!("Portal accent color unavailable; using fallback accent");
    }

    apply_palette(cx, &build_palette(dark, accent));
}
