#[cfg(target_os = "windows")]
pub mod windows_theme;

#[cfg(target_os = "linux")]
pub mod linux_theme;
#[cfg(target_os = "macos")]
pub mod macos_theme;

use gpui::Hsla;
#[cfg(target_os = "linux")]
pub use linux_theme::apply_linux_system_theme;
#[cfg(target_os = "macos")]
pub use macos_theme::apply_macos_system_theme;

/// Converts RGBA (0.0–1.0) to a `gpui::Hsla`.
///
/// GPUI's `Hsla` expects h as a fraction of a full turn (0.0–1.0),
/// s and l in 0.0–1.0, and a in 0.0–1.0.
#[allow(unused)]
pub(crate) fn rgba_to_hsla(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
) -> Hsla {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a,
        };
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    Hsla { h, s, l, a }
}
