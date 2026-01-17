//! Status bar component for displaying keyboard shortcuts.

use cursive::view::Resizable;
use cursive::views::{LinearLayout, TextView};

/// Keyboard shortcut hint for the status bar.
pub struct KeyHint {
    pub key: &'static str,
    pub action: &'static str,
}

impl KeyHint {
    pub const fn new(key: &'static str, action: &'static str) -> Self {
        Self { key, action }
    }
}

/// Build a status bar from a list of key hints.
pub fn build_status_bar(hints: &[KeyHint]) -> LinearLayout {
    let hint_text = hints
        .iter()
        .map(|h| format!("{}: {}", h.key, h.action))
        .collect::<Vec<_>>()
        .join(" │ ");

    LinearLayout::horizontal()
        .child(TextView::new(hint_text).full_width())
}

/// Common key hints for form views.
pub mod hints {
    use super::KeyHint;

    pub const TAB: KeyHint = KeyHint::new("Tab", "Next");
    pub const SHIFT_TAB: KeyHint = KeyHint::new("S-Tab", "Prev");
    pub const ESC: KeyHint = KeyHint::new("Esc", "Back");
    pub const ENTER: KeyHint = KeyHint::new("Enter", "Select");
    pub const CTRL_Q: KeyHint = KeyHint::new("C-q", "Quit");
}
