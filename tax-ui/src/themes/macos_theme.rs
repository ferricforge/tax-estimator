use gpui::{App, Hsla};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSColor,
    NSColorSpace,
};

use super::{SystemPalette, ThemeMode, apply_palette, hex, rgba_to_hsla};

/// Converts an `NSColor` to `Hsla` through sRGB.
///
/// Dynamic system colours have no components until they are resolved in a
/// concrete colour space, so `fallback` covers the case where conversion
/// is not possible.
fn nscolor_or(
    color: &NSColor,
    fallback: Hsla,
) -> Hsla {
    let Some(converted) = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) else {
        return fallback;
    };

    rgba_to_hsla(
        converted.redComponent() as f32,
        converted.greenComponent() as f32,
        converted.blueComponent() as f32,
        converted.alphaComponent() as f32,
    )
}

/// The appearance `mode` asks for, or `None` to follow the operating system.
fn appearance_for(mode: ThemeMode) -> Option<Retained<NSAppearance>> {
    match mode {
        ThemeMode::System => None,
        ThemeMode::Light => NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua }),
        ThemeMode::Dark => NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }),
    }
}

/// Runs `read` with `appearance` installed as the current appearance, so the
/// dynamic system colors resolve to it, then restores the previous value.
///
/// `currentAppearance` and `setCurrentAppearance` are deprecated in favour of
/// `performAsCurrentDrawingAppearance`, which takes an Objective-C block. The
/// block form needs a `block2` dependency and feature flags that have not been
/// checked here.
#[allow(deprecated)]
fn with_current_appearance<R>(
    appearance: &NSAppearance,
    read: impl FnOnce() -> R,
) -> R {
    let previous = NSAppearance::currentAppearance();

    // SAFETY: both arguments are valid references or `None`. The call only
    // changes the appearance used for drawing on this thread.
    unsafe { NSAppearance::setCurrentAppearance(Some(appearance)) };
    let result = read();
    unsafe { NSAppearance::setCurrentAppearance(previous.as_deref()) };

    result
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

/// Installs `mode` as the application appearance.
///
/// Window frames follow this at once, and it decides what
/// `effectiveAppearance` reports when the palette is read. Call it while no
/// `App` borrow is held: AppKit notifies the open windows during the call and
/// gpui's handler borrows the application.
pub fn install_appearance(mode: ThemeMode) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    NSApplication::sharedApplication(mtm).setAppearance(appearance_for(mode).as_deref());
}

/// Applies macOS system colours to the gpui-component global [`Theme`].
///
/// The palette is read with the application's effective appearance installed
/// as the current one, because the dynamic system colors otherwise resolve
/// against a nil appearance, which falls back to Aqua. The effective
/// appearance already reflects the mode that [`install_appearance`] set, or
/// the system setting when nothing is forced, so `_mode` is not read here.
pub fn apply_macos_system_theme(
    cx: &mut App,
    _mode: ThemeMode,
) {
    let Some(mtm) = MainThreadMarker::new() else {
        apply_palette(cx, &build_palette());
        return;
    };

    let effective = NSApplication::sharedApplication(mtm).effectiveAppearance();
    apply_palette(cx, &with_current_appearance(&effective, build_palette));
}
