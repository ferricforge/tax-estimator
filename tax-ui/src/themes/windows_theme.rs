use gpui::{App, Hsla};
use windows::Win32::Graphics::Dwm::DwmGetColorizationColor;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows::core::PCWSTR;

use super::{SystemPalette, apply_palette, hex, rgba_to_hsla};

// ── Registry helpers ──────────────────────────────────────────────

/// Reads a `REG_DWORD` from `HKCU\<subkey>\<value_name>`.
///
/// Returns `None` when the key or value does not exist, or when
/// the read fails for any other reason.
fn read_hkcu_dword(
    subkey: PCWSTR,
    value_name: PCWSTR,
) -> Option<u32> {
    unsafe {
        let mut key_handle = HKEY::default();
        let open_result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey,
            Some(0),
            KEY_READ,
            &mut key_handle,
        );
        if open_result.0 != 0 {
            return None;
        }

        let mut data: u32 = 0;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let query_result = RegQueryValueExW(
            key_handle,
            value_name,
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut data_size),
        );
        let _ = RegCloseKey(key_handle);

        if query_result.0 != 0 {
            return None;
        }
        Some(data)
    }
}

// ── System queries ────────────────────────────────────────────────

/// Returns `true` when Windows is set to dark app mode.
///
/// Reads `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize`.
/// A value of `0` for `AppsUseLightTheme` means dark mode is active.
fn is_dark_mode() -> bool {
    let theme_value = read_hkcu_dword(
        windows::core::w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        windows::core::w!("AppsUseLightTheme"),
    );
    // 0 → dark, 1 → light, absent → assume light.
    theme_value == Some(0)
}

/// Reads the user's accent colour from the DWM registry key.
///
/// `HKCU\SOFTWARE\Microsoft\Windows\DWM\AccentColor` stores the
/// colour as a DWORD in **ABGR** byte order (alpha–blue–green–red).
fn accent_from_registry() -> Option<Hsla> {
    let raw = read_hkcu_dword(
        windows::core::w!("SOFTWARE\\Microsoft\\Windows\\DWM"),
        windows::core::w!("AccentColor"),
    )?;
    // ABGR layout: byte 0 = R, byte 1 = G, byte 2 = B, byte 3 = A
    let red = (raw & 0xFF) as f32 / 255.0;
    let green = ((raw >> 8) & 0xFF) as f32 / 255.0;
    let blue = ((raw >> 16) & 0xFF) as f32 / 255.0;
    Some(rgba_to_hsla(red, green, blue, 1.0))
}

/// Falls back to the DWM colourisation colour (**ARGB** byte order).
///
/// The second parameter's concrete type varies across `windows` crate
/// versions (`BOOL`, `bool`, etc.).  We use `Default::default()` with
/// type inference so this compiles regardless of the crate's choice.
fn accent_from_dwm() -> Option<Hsla> {
    let mut argb: u32 = 0;
    let mut opaque_blend = Default::default();
    // SAFETY: both pointers are valid stack locals.
    unsafe {
        DwmGetColorizationColor(&mut argb, &mut opaque_blend).ok()?;
    }
    let red = ((argb >> 16) & 0xFF) as f32 / 255.0;
    let green = ((argb >> 8) & 0xFF) as f32 / 255.0;
    let blue = (argb & 0xFF) as f32 / 255.0;
    Some(rgba_to_hsla(red, green, blue, 1.0))
}

/// Best-effort accent colour: registry → DWM → default Windows blue.
fn accent_color() -> Hsla {
    accent_from_registry()
        .or_else(accent_from_dwm)
        .unwrap_or(hex(DEFAULT_BLUE))
}

// ── Fluent Design palette constants ───────────────────────────────
// Colours are taken from the WinUI 3 / Fluent Design system so
// the application feels native on Windows 10 and 11.

// Light-mode base surfaces
const LIGHT_WINDOW_BG: u32 = 0xF3F3F3;
const LIGHT_CONTROL_BG: u32 = 0xFFFFFF;
const LIGHT_LABEL: u32 = 0x1A1A1A;
const LIGHT_SECONDARY_LABEL: u32 = 0x616161;
const LIGHT_TERTIARY_LABEL: u32 = 0x9E9E9E;
const LIGHT_SEPARATOR: u32 = 0xD1D1D1;
const LIGHT_UNEMPHASIZED: u32 = 0xE5E5E5;
const LIGHT_LINK: u32 = 0x005A9E;

// Dark-mode base surfaces
const DARK_WINDOW_BG: u32 = 0x202020;
const DARK_CONTROL_BG: u32 = 0x2D2D2D;
const DARK_LABEL: u32 = 0xFFFFFF;
const DARK_SECONDARY_LABEL: u32 = 0xC5C5C5;
const DARK_TERTIARY_LABEL: u32 = 0x9E9E9E;
const DARK_SEPARATOR: u32 = 0x3D3D3D;
const DARK_UNEMPHASIZED: u32 = 0x383838;
const DARK_LINK: u32 = 0x4CC2FF;

// Windows 11 standard semantic colours (shared across modes)
const WIN_RED: u32 = 0xC42B1C;
const WIN_ORANGE: u32 = 0xF7630C;
const WIN_YELLOW: u32 = 0xFCD116;
const WIN_GREEN: u32 = 0x16C60C;
const WIN_TEAL: u32 = 0x00B7C3;
const WIN_PURPLE: u32 = 0x886CE4;
const WIN_PINK: u32 = 0xE3008C;
const DEFAULT_BLUE: u32 = 0x0078D4;

// ── Palette builder ───────────────────────────────────────────────

/// Selects the correct constant for the current mode.
fn pick(
    dark: bool,
    light_value: u32,
    dark_value: u32,
) -> Hsla {
    hex(if dark { dark_value } else { light_value })
}

/// Builds a [`SystemPalette`] that mirrors the current Windows theme.
fn build_palette() -> SystemPalette {
    let dark = is_dark_mode();
    let accent = accent_color();

    // Selection background: a desaturated tint of the accent colour
    // so that selected text remains legible.
    let selected_text_bg = Hsla {
        s: accent.s * 0.55,
        l: if dark { 0.32 } else { 0.82 },
        a: 1.0,
        ..accent
    };

    SystemPalette {
        accent,
        accent_foreground: hex(0xFFFFFF),
        window_bg: pick(dark, LIGHT_WINDOW_BG, DARK_WINDOW_BG),
        control_bg: pick(dark, LIGHT_CONTROL_BG, DARK_CONTROL_BG),
        label: pick(dark, LIGHT_LABEL, DARK_LABEL),
        secondary_label: pick(dark, LIGHT_SECONDARY_LABEL, DARK_SECONDARY_LABEL),
        tertiary_label: pick(dark, LIGHT_TERTIARY_LABEL, DARK_TERTIARY_LABEL),
        separator: pick(dark, LIGHT_SEPARATOR, DARK_SEPARATOR),
        selected_text_bg,
        keyboard_focus: accent,
        link: pick(dark, LIGHT_LINK, DARK_LINK),
        unemphasized_bg: pick(dark, LIGHT_UNEMPHASIZED, DARK_UNEMPHASIZED),
        red: hex(WIN_RED),
        orange: hex(WIN_ORANGE),
        yellow: hex(WIN_YELLOW),
        green: hex(WIN_GREEN),
        teal: hex(WIN_TEAL),
        blue: hex(DEFAULT_BLUE),
        purple: hex(WIN_PURPLE),
        pink: hex(WIN_PINK),
    }
}

/// Applies Windows system colours to the gpui-component global [`Theme`].
///
/// Call after `gpui_component::init(cx)`.
pub fn apply_windows_system_theme(cx: &mut App) {
    apply_palette(cx, &build_palette());
}
