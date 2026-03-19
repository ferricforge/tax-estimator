use gpui::Hsla;

// ---------------------------------------------------------------------------
// Display field colors
// ---------------------------------------------------------------------------

/// Background for read-only calculated display fields.
pub const DISPLAY_FIELD_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.15,
    a: 1.0,
};

/// Border color for read-only calculated display fields.
pub const DISPLAY_FIELD_BORDER: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.35,
    a: 1.0,
};

/// Text color for read-only calculated display fields.
pub const DISPLAY_FIELD_TEXT: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.85,
    a: 1.0,
};

// ---------------------------------------------------------------------------
// Section header colors
// ---------------------------------------------------------------------------

/// Border and text color for section header rows.
pub const HEADER_ACCENT: Hsla = Hsla {
    // 336° mapped to 0..1
    h: 0.933,
    s: 0.75,
    l: 0.5,
    a: 1.0,
};
