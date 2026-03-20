use gpui::{App, Hsla};
use objc2_app_kit::{NSColor, NSColorSpace};

use super::{SystemPalette, apply_palette, hex, rgba_to_hsla};

/// Converts an `NSColor` to `Hsla`, going through sRGB first.
fn nscolor_to_hsla(color: &NSColor) -> Option<Hsla> {
    let srgb_space = NSColorSpace::sRGBColorSpace();
    let converted = color.colorUsingColorSpace(&srgb_space)?;
    Some(rgba_to_hsla(
        converted.redComponent() as f32,
        converted.greenComponent() as f32,
        converted.blueComponent() as f32,
        converted.alphaComponent() as f32,
    ))
}

/// Converts an `NSColor` to `Hsla`, falling back to `fallback` on failure.
fn nscolor_or(
    color: &NSColor,
    fallback: Hsla,
) -> Hsla {
    nscolor_to_hsla(color).unwrap_or(fallback)
}

/// Builds a [`SystemPalette`] from the current macOS appearance.
fn build_palette() -> SystemPalette {
    // Sensible mid-grey fallback so the UI is never invisible.
    let mid_grey = hex(0x808080);

    SystemPalette {
        accent: nscolor_or(&NSColor::controlAccentColor(), mid_grey),
        accent_foreground: nscolor_or(&NSColor::alternateSelectedControlTextColor(), hex(0xFFFFFF)),
        window_bg: nscolor_or(&NSColor::windowBackgroundColor(), mid_grey),
        control_bg: nscolor_or(&NSColor::controlBackgroundColor(), mid_grey),
        label: nscolor_or(&NSColor::labelColor(), hex(0x000000)),
        secondary_label: nscolor_or(&NSColor::secondaryLabelColor(), mid_grey),
        tertiary_label: nscolor_or(&NSColor::tertiaryLabelColor(), mid_grey),
        separator: nscolor_or(&NSColor::separatorColor(), mid_grey),
        selected_text_bg: nscolor_or(&NSColor::selectedTextBackgroundColor(), mid_grey),
        keyboard_focus: nscolor_or(&NSColor::keyboardFocusIndicatorColor(), mid_grey),
        link: nscolor_or(&NSColor::linkColor(), hex(0x0068DA)),
        unemphasized_bg: nscolor_or(
            &NSColor::unemphasizedSelectedContentBackgroundColor(),
            mid_grey,
        ),
        red: nscolor_or(&NSColor::systemRedColor(), hex(0xFF3B30)),
        orange: nscolor_or(&NSColor::systemOrangeColor(), hex(0xFF9500)),
        yellow: nscolor_or(&NSColor::systemYellowColor(), hex(0xFFCC00)),
        green: nscolor_or(&NSColor::systemGreenColor(), hex(0x28CD41)),
        teal: nscolor_or(&NSColor::systemTealColor(), hex(0x59ADC4)),
        blue: nscolor_or(&NSColor::systemBlueColor(), hex(0x007AFF)),
        purple: nscolor_or(&NSColor::systemPurpleColor(), hex(0xAF52DE)),
        pink: nscolor_or(&NSColor::systemPinkColor(), hex(0xFF2D55)),
    }
}

/// Applies macOS system colours to the gpui-component global [`Theme`].
///
/// Call after `gpui_component::init(cx)`.
pub fn apply_macos_system_theme(cx: &mut App) {
    apply_palette(cx, &build_palette());
}
