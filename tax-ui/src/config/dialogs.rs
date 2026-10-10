//! The `[dialogs]` section: confirmations the user has turned off.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Confirmations the user has chosen not to see again, stored in the
/// `[dialogs]` section of the configuration.
///
/// Each entry is the key of one confirmation, such as the Clear confirmation
/// of a worksheet. A missing section means every confirmation is shown.
///
/// ```toml
/// [dialogs]
/// suppressed = ["qbi-form-clear"]
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DialogsConfig {
    /// Keys of the confirmations that are hidden, in sorted order.
    suppressed: BTreeSet<String>,
}

impl DialogsConfig {
    /// Returns `true` when the confirmation named `key` is hidden.
    pub fn is_suppressed(
        &self,
        key: &str,
    ) -> bool {
        self.suppressed.contains(key)
    }

    /// Hides the confirmation named `key`.
    pub fn suppress(
        &mut self,
        key: &str,
    ) {
        self.suppressed.insert(key.to_string());
    }

    /// Shows every confirmation again, and returns how many were hidden.
    pub fn reset(&mut self) -> usize {
        let count = self.suppressed.len();
        self.suppressed.clear();
        count
    }

    /// The number of confirmations that are hidden.
    pub fn suppressed_count(&self) -> usize {
        self.suppressed.len()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn nothing_is_suppressed_by_default() {
        let config = DialogsConfig::default();

        assert_eq!(config.suppressed_count(), 0);
        assert!(!config.is_suppressed("qbi-form-clear"));
    }

    #[test]
    fn suppress_hides_one_confirmation() {
        let mut config = DialogsConfig::default();
        config.suppress("qbi-form-clear");

        assert!(config.is_suppressed("qbi-form-clear"));
        assert!(!config.is_suppressed("se-worksheet-clear"));
        assert_eq!(config.suppressed_count(), 1);
    }

    #[test]
    fn suppressing_the_same_key_twice_records_one_entry() {
        let mut config = DialogsConfig::default();
        config.suppress("qbi-form-clear");
        config.suppress("qbi-form-clear");

        assert_eq!(config.suppressed_count(), 1);
    }

    #[test]
    fn reset_reports_how_many_were_hidden() {
        let mut config = DialogsConfig::default();
        config.suppress("qbi-form-clear");
        config.suppress("se-worksheet-clear");

        assert_eq!(config.reset(), 2);
        assert_eq!(config.suppressed_count(), 0);
    }

    #[test]
    fn section_round_trips_through_toml() {
        let mut original = DialogsConfig::default();
        original.suppress("qbi-form-clear");

        let text = toml::to_string_pretty(&original).expect("section must serialize");
        let parsed: DialogsConfig = toml::from_str(&text).expect("section must parse");

        assert_eq!(parsed, original);
    }

    #[test]
    fn empty_section_hides_nothing() {
        let config: DialogsConfig = toml::from_str("").expect("section must parse");

        assert_eq!(config, DialogsConfig::default());
    }
}
