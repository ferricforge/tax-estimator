use gpui::{App, Hsla};
use gpui_component::Theme;
use objc2_app_kit::{NSColor, NSColorSpace};

/// Extracts RGBA components from an NSColor, converting to sRGB first.
///
/// Returns `None` if the color cannot be converted to sRGB (e.g. pattern colors).
fn nscolor_to_rgba(color: &NSColor) -> Option<(f32, f32, f32, f32)> {
    let srgb = NSColorSpace::sRGBColorSpace();
    let converted = color.colorUsingColorSpace(&srgb)?;

    let r = converted.redComponent() as f32;
    let g = converted.greenComponent() as f32;
    let b = converted.blueComponent() as f32;
    let a = converted.alphaComponent() as f32;

    Some((r, g, b, a))
}

/// Converts RGBA (0.0–1.0) to a `gpui::Hsla`.
///
/// GPUI's `Hsla` expects h as a fraction of a full turn (0.0–1.0),
/// s and l in 0.0–1.0, and a in 0.0–1.0.
fn rgba_to_hsla(
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

/// Converts an `NSColor` to a `gpui::Hsla`.
///
/// Returns `None` if the color cannot be represented in sRGB.
fn nscolor_to_hsla(color: &NSColor) -> Option<Hsla> {
    let (r, g, b, a) = nscolor_to_rgba(color)?;
    Some(rgba_to_hsla(r, g, b, a))
}

/// Assigns an `NSColor` to an `Hsla` field if conversion succeeds.
fn apply(
    target: &mut Hsla,
    color: &NSColor,
) {
    if let Some(hsla) = nscolor_to_hsla(color) {
        *target = hsla;
    }
}

/// Derives a lighter variant of an `Hsla` color by raising lightness.
fn lighter(base: Hsla) -> Hsla {
    Hsla {
        l: (base.l + 0.15).min(1.0),
        ..base
    }
}

/// Derives a hover variant by shifting lightness toward 50%.
fn hover_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.05 } else { 0.05 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

/// Derives an active/pressed variant by shifting lightness further.
fn active_variant(base: Hsla) -> Hsla {
    let shift = if base.l > 0.5 { -0.10 } else { 0.10 };
    Hsla {
        l: (base.l + shift).clamp(0.0, 1.0),
        ..base
    }
}

/// Applies macOS system colors to the gpui-component global Theme.
///
/// Reads the user's current system appearance (accent color, label colors,
/// window background, etc.) and maps them onto every `ThemeColor` field
/// so that gpui-component widgets reflect the native macOS palette.
///
/// Call after `gpui_component::init(cx)` in your app setup.
pub fn apply_macos_system_theme(cx: &mut App) {
    let colors = &mut Theme::global_mut(cx).colors;

    // ── Read system colors once ───────────────────────────────────
    let accent = NSColor::controlAccentColor();
    let window_bg = NSColor::windowBackgroundColor();
    let control_bg = NSColor::controlBackgroundColor();
    let label = NSColor::labelColor();
    let secondary_label = NSColor::secondaryLabelColor();
    let tertiary_label = NSColor::tertiaryLabelColor();
    let separator = NSColor::separatorColor();
    let selected_text_bg = NSColor::selectedTextBackgroundColor();
    let keyboard_focus = NSColor::keyboardFocusIndicatorColor();
    let link = NSColor::linkColor();
    let alt_selected_text = NSColor::alternateSelectedControlTextColor();
    let unemphasized_bg = NSColor::unemphasizedSelectedContentBackgroundColor();

    let sys_red = NSColor::systemRedColor();
    let sys_orange = NSColor::systemOrangeColor();
    let sys_yellow = NSColor::systemYellowColor();
    let sys_green = NSColor::systemGreenColor();
    let sys_teal = NSColor::systemTealColor();
    let sys_blue = NSColor::systemBlueColor();
    let sys_purple = NSColor::systemPurpleColor();
    let sys_pink = NSColor::systemPinkColor();

    // ── Primary ───────────────────────────────────────────────────
    apply(&mut colors.primary, &accent);
    apply(&mut colors.primary_foreground, &alt_selected_text);
    if let Some(h) = nscolor_to_hsla(&accent) {
        colors.primary_hover = hover_variant(h);
        colors.primary_active = active_variant(h);
    }

    // ── Accent (hover highlights on menu/list items) ──────────────
    apply(&mut colors.accent, &unemphasized_bg);
    apply(&mut colors.accent_foreground, &label);

    // ── Background / foreground ───────────────────────────────────
    apply(&mut colors.background, &window_bg);
    apply(&mut colors.foreground, &label);

    // ── Secondary ─────────────────────────────────────────────────
    apply(&mut colors.secondary, &unemphasized_bg);
    apply(&mut colors.secondary_foreground, &secondary_label);
    if let Some(h) = nscolor_to_hsla(&unemphasized_bg) {
        colors.secondary_hover = hover_variant(h);
        colors.secondary_active = active_variant(h);
    }

    // ── Muted ─────────────────────────────────────────────────────
    apply(&mut colors.muted, &unemphasized_bg);
    apply(&mut colors.muted_foreground, &secondary_label);

    // ── Popover ───────────────────────────────────────────────────
    apply(&mut colors.popover, &control_bg);
    apply(&mut colors.popover_foreground, &label);

    // ── Borders / input / ring ────────────────────────────────────
    apply(&mut colors.border, &separator);
    apply(&mut colors.input, &separator);
    apply(&mut colors.ring, &keyboard_focus);

    // ── Selection / caret ─────────────────────────────────────────
    apply(&mut colors.selection, &selected_text_bg);
    apply(&mut colors.caret, &accent);

    // ── Link ──────────────────────────────────────────────────────
    apply(&mut colors.link, &link);
    if let Some(h) = nscolor_to_hsla(&link) {
        colors.link_hover = hover_variant(h);
        colors.link_active = active_variant(h);
    }

    // ── Danger (red) ──────────────────────────────────────────────
    apply(&mut colors.danger, &sys_red);
    apply(&mut colors.danger_foreground, &alt_selected_text);
    if let Some(h) = nscolor_to_hsla(&sys_red) {
        colors.danger_hover = hover_variant(h);
        colors.danger_active = active_variant(h);
    }

    // ── Success (green) ───────────────────────────────────────────
    apply(&mut colors.success, &sys_green);
    apply(&mut colors.success_foreground, &alt_selected_text);
    if let Some(h) = nscolor_to_hsla(&sys_green) {
        colors.success_hover = hover_variant(h);
        colors.success_active = active_variant(h);
    }

    // ── Warning (orange) ──────────────────────────────────────────
    apply(&mut colors.warning, &sys_orange);
    apply(&mut colors.warning_foreground, &alt_selected_text);
    if let Some(h) = nscolor_to_hsla(&sys_orange) {
        colors.warning_hover = hover_variant(h);
        colors.warning_active = active_variant(h);
    }

    // ── Info (blue) ───────────────────────────────────────────────
    apply(&mut colors.info, &sys_blue);
    apply(&mut colors.info_foreground, &alt_selected_text);
    if let Some(h) = nscolor_to_hsla(&sys_blue) {
        colors.info_hover = hover_variant(h);
        colors.info_active = active_variant(h);
    }

    // ── Named palette colors ──────────────────────────────────────
    apply(&mut colors.red, &sys_red);
    apply(&mut colors.green, &sys_green);
    apply(&mut colors.blue, &sys_blue);
    apply(&mut colors.yellow, &sys_yellow);
    apply(&mut colors.cyan, &sys_teal);
    apply(&mut colors.magenta, &sys_pink);

    if let Some(h) = nscolor_to_hsla(&sys_red) {
        colors.red_light = lighter(h);
    }
    if let Some(h) = nscolor_to_hsla(&sys_green) {
        colors.green_light = lighter(h);
    }
    if let Some(h) = nscolor_to_hsla(&sys_blue) {
        colors.blue_light = lighter(h);
    }
    if let Some(h) = nscolor_to_hsla(&sys_yellow) {
        colors.yellow_light = lighter(h);
    }
    if let Some(h) = nscolor_to_hsla(&sys_teal) {
        colors.cyan_light = lighter(h);
    }
    if let Some(h) = nscolor_to_hsla(&sys_pink) {
        colors.magenta_light = lighter(h);
    }

    // ── Candlestick chart ─────────────────────────────────────────
    apply(&mut colors.bullish, &sys_green);
    apply(&mut colors.bearish, &sys_red);

    // ── Sidebar ───────────────────────────────────────────────────
    apply(&mut colors.sidebar, &window_bg);
    apply(&mut colors.sidebar_foreground, &label);
    apply(&mut colors.sidebar_border, &separator);
    apply(&mut colors.sidebar_accent, &unemphasized_bg);
    apply(&mut colors.sidebar_accent_foreground, &label);
    apply(&mut colors.sidebar_primary, &accent);
    apply(&mut colors.sidebar_primary_foreground, &alt_selected_text);

    // ── Title bar ─────────────────────────────────────────────────
    apply(&mut colors.title_bar, &window_bg);
    apply(&mut colors.title_bar_border, &separator);

    // ── Window border (Linux only, harmless to set) ───────────────
    apply(&mut colors.window_border, &separator);

    // ── Overlay ───────────────────────────────────────────────────
    apply(
        &mut colors.overlay,
        &NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.4),
    );

    // ── List ──────────────────────────────────────────────────────
    apply(&mut colors.list, &control_bg);
    apply(&mut colors.list_hover, &unemphasized_bg);
    apply(&mut colors.list_active, &accent);
    apply(&mut colors.list_active_border, &accent);
    apply(&mut colors.list_head, &window_bg);
    if let Some(h) = nscolor_to_hsla(&control_bg) {
        colors.list_even = hover_variant(h);
    }

    // ── Table ─────────────────────────────────────────────────────
    apply(&mut colors.table, &control_bg);
    apply(&mut colors.table_hover, &unemphasized_bg);
    apply(&mut colors.table_active, &accent);
    apply(&mut colors.table_active_border, &accent);
    apply(&mut colors.table_head, &window_bg);
    apply(&mut colors.table_head_foreground, &label);
    apply(&mut colors.table_row_border, &separator);
    if let Some(h) = nscolor_to_hsla(&control_bg) {
        colors.table_even = hover_variant(h);
    }

    // ── Tab ───────────────────────────────────────────────────────
    apply(&mut colors.tab, &window_bg);
    apply(&mut colors.tab_foreground, &secondary_label);
    apply(&mut colors.tab_active, &control_bg);
    apply(&mut colors.tab_active_foreground, &label);
    apply(&mut colors.tab_bar, &window_bg);
    apply(&mut colors.tab_bar_segmented, &unemphasized_bg);

    // ── Scrollbar ─────────────────────────────────────────────────
    apply(&mut colors.scrollbar, &window_bg);
    apply(&mut colors.scrollbar_thumb, &tertiary_label);
    apply(&mut colors.scrollbar_thumb_hover, &secondary_label);

    // ── Slider ────────────────────────────────────────────────────
    apply(&mut colors.slider_bar, &unemphasized_bg);
    apply(&mut colors.slider_thumb, &control_bg);

    // ── Switch ────────────────────────────────────────────────────
    apply(&mut colors.switch, &unemphasized_bg);
    apply(&mut colors.switch_thumb, &control_bg);

    // ── Progress bar ──────────────────────────────────────────────
    apply(&mut colors.progress_bar, &accent);

    // ── Skeleton ──────────────────────────────────────────────────
    apply(&mut colors.skeleton, &unemphasized_bg);

    // ── Accordion ─────────────────────────────────────────────────
    apply(&mut colors.accordion, &control_bg);
    apply(&mut colors.accordion_hover, &unemphasized_bg);

    // ── GroupBox ───────────────────────────────────────────────────
    apply(&mut colors.group_box, &control_bg);
    apply(&mut colors.group_box_foreground, &label);

    // ── DescriptionList ───────────────────────────────────────────
    apply(&mut colors.description_list_label, &unemphasized_bg);
    apply(
        &mut colors.description_list_label_foreground,
        &secondary_label,
    );

    // ── Drag / drop ───────────────────────────────────────────────
    apply(&mut colors.drag_border, &accent);
    apply(&mut colors.drop_target, &unemphasized_bg);

    // ── Tiles ─────────────────────────────────────────────────────
    apply(&mut colors.tiles, &control_bg);

    // ── Chart palette ─────────────────────────────────────────────
    apply(&mut colors.chart_1, &sys_blue);
    apply(&mut colors.chart_2, &sys_green);
    apply(&mut colors.chart_3, &sys_orange);
    apply(&mut colors.chart_4, &sys_purple);
    apply(&mut colors.chart_5, &sys_teal);
}
