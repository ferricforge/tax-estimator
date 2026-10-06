//! Appearance settings, stored in the `[appearance]` section.

use serde::{Deserialize, Serialize};

use crate::themes::ThemeMode;

/// Appearance settings stored in the `[appearance]` section of the
/// configuration.
///
/// A missing section or field uses the defaults, so a file written before
/// this section existed follows the operating system.
///
/// ```toml
/// [appearance]
/// theme = "system"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// `system`, `light`, or `dark`.
    pub theme: ThemeMode,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn default_follows_the_system() {
        assert_eq!(AppearanceConfig::default().theme, ThemeMode::System);
    }

    #[test]
    fn empty_section_uses_the_default_theme() {
        let config: AppearanceConfig = toml::from_str("").expect("section must parse");

        assert_eq!(config.theme, ThemeMode::System);
    }

    #[test]
    fn theme_names_are_read_in_lowercase() {
        let names = ["system", "light", "dark"];
        let expected = ThemeMode::ALL;

        for (name, mode) in names.into_iter().zip(expected) {
            let text = format!("theme = \"{name}\"");
            let config: AppearanceConfig = toml::from_str(&text).expect("section must parse");

            assert_eq!(config.theme, mode);
        }
    }

    #[test]
    fn unknown_theme_name_is_rejected() {
        let result = toml::from_str::<AppearanceConfig>("theme = \"sepia\"");

        assert!(result.is_err());
    }

    #[test]
    fn theme_round_trips_through_toml() {
        let original = AppearanceConfig {
            theme: ThemeMode::Dark,
        };

        let text = toml::to_string_pretty(&original).expect("section must serialize");
        let parsed: AppearanceConfig = toml::from_str(&text).expect("section must parse");

        assert_eq!(parsed, original);
    }
}
