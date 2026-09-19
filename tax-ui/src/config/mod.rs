mod store;

pub use store::{ConfigStore, TomlConfigStore};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use crate::logging::{self, LogSettings, SourcePathDisplay};

/// Application name used for the configuration directory and the default log
/// file name.
const APP_NAME: &str = "TaxEstimator";

/// File extension of the default log file.
const LOG_FILE_EXTENSION: &str = "log";

// ---------------------------------------------------------------------------
// DatabaseBackend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseBackend {
    #[default]
    Sqlite,
    // Postgres,
    // MySql,
}

impl DatabaseBackend {
    /// Canonical lowercase name. Matches the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
        }
    }
}

impl fmt::Display for DatabaseBackend {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// `let s: &str = backend.into();`
impl From<DatabaseBackend> for &'static str {
    fn from(b: DatabaseBackend) -> Self {
        b.as_str()
    }
}

// `let s: String = backend.into();`
impl From<DatabaseBackend> for String {
    fn from(b: DatabaseBackend) -> Self {
        b.as_str().to_owned()
    }
}

impl FromStr for DatabaseBackend {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "sqlite" => Ok(Self::Sqlite),
            other => anyhow::bail!("unknown database backend: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// LoggingConfig
// ---------------------------------------------------------------------------

/// Logging settings stored in the `[logging]` section of the configuration.
///
/// The section and each of its fields are optional; missing values use the
/// defaults. The `RUST_LOG` environment variable overrides `level` and
/// `application_only`.
///
/// ```toml
/// [logging]
/// level = "info"
/// application_only = true
/// stdout = true
/// file_enabled = false
/// file_path = "TaxEstimator.log"
/// source_path = "full"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Bare level (`error`, `warn`, `info`, `debug`, `trace`) or a full filter
    /// directive.
    pub level: String,

    /// Whether a bare [`level`](Self::level) applies only to the application
    /// workspace crates. When false, a bare level applies to every target.
    /// Full filter directives define their own target scopes and ignore this
    /// setting.
    pub application_only: bool,

    /// Whether log output is written to stdout.
    pub stdout: bool,

    /// Whether log output is written to [`file_path`](Self::file_path).
    pub file_enabled: bool,

    /// Log file location. A relative path is resolved against the current
    /// working directory. Missing parent directories are created when file
    /// logging starts.
    pub file_path: PathBuf,

    /// How the source location of each event is shown: `full`, `short`,
    /// `file_name`, or `hidden`.
    pub source_path: SourcePathDisplay,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: logging::default_level().to_string(),
            application_only: true,
            stdout: true,
            file_enabled: false,
            file_path: PathBuf::from(APP_NAME).with_extension(LOG_FILE_EXTENSION),
            source_path: SourcePathDisplay::default(),
        }
    }
}

impl LoggingConfig {
    /// Returns these settings in the form the logging module applies.
    pub fn settings(&self) -> LogSettings<'_> {
        LogSettings {
            level: &self.level,
            application_only: self.application_only,
            stdout: self.stdout,
            file: self.file_enabled.then_some(self.file_path.as_path()),
            source_path: self.source_path,
        }
    }
}

// ---------------------------------------------------------------------------
// AppConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub database_backend: DatabaseBackend,

    /// Logging settings. The `[logging]` section is optional in the file.
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "taxes.db".into(),
            database_backend: DatabaseBackend::Sqlite,
            logging: LoggingConfig::default(),
        }
    }
}

impl Global for AppConfig {}

/// Opaque wrapper so a `dyn ConfigStore` can live in gpui's global map.
struct ConfigStoreHandle(Box<dyn ConfigStore>);
impl Global for ConfigStoreHandle {}

impl AppConfig {
    /// Installs an already-loaded configuration and its optional persistence
    /// store as gpui globals.
    pub fn install(
        cx: &mut App,
        config: Self,
        store: Option<Box<dyn ConfigStore>>,
    ) {
        cx.set_global(config);
        if let Some(store) = store {
            cx.set_global(ConfigStoreHandle(store));
        }
    }

    /// Load (or create) config via `store` and install both the config
    /// and the store as gpui globals.
    pub fn init(
        cx: &mut App,
        store: impl ConfigStore,
    ) -> anyhow::Result<()> {
        let config = store.load_or_init()?;
        Self::install(cx, config, Some(Box::new(store)));
        Ok(())
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn try_get(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    /// Mutate in place. `global_mut` marks the global dirty so any
    /// `observe_global::<AppConfig>` subscribers are notified.
    pub fn update<R>(
        cx: &mut App,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        f(cx.global_mut::<Self>())
    }

    /// Persist the current in-memory config through the registered store.
    pub fn save(cx: &App) -> anyhow::Result<()> {
        let store = cx
            .try_global::<ConfigStoreHandle>()
            .ok_or_else(|| anyhow::anyhow!("no ConfigStore registered"))?;
        store.0.save(cx.global::<Self>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    /// Database settings that every configuration file must contain.
    const DATABASE_SECTION: &str = r#"
database_url = "taxes.db"
database_backend = "sqlite"
"#;

    /// Parses a configuration with the database settings and `logging_lines`
    /// as the `[logging]` section.
    fn parse_with_logging_section(logging_lines: &str) -> AppConfig {
        let text = format!("{DATABASE_SECTION}\n[logging]\n{logging_lines}\n");
        toml::from_str(&text).expect("config must parse")
    }

    #[test]
    fn logging_config_defaults_match_the_specification() {
        let expected = LoggingConfig {
            level: "info".to_string(),
            application_only: true,
            stdout: true,
            file_enabled: false,
            file_path: PathBuf::from("TaxEstimator.log"),
            source_path: SourcePathDisplay::Full,
        };

        assert_eq!(LoggingConfig::default(), expected);
    }

    #[test]
    fn app_config_without_logging_section_uses_logging_defaults() {
        let config: AppConfig = toml::from_str(DATABASE_SECTION).expect("config must parse");
        assert_eq!(config.logging, LoggingConfig::default());
    }

    #[test]
    fn logging_config_fills_missing_fields_with_defaults() {
        let config = parse_with_logging_section("file_enabled = true");
        let expected = LoggingConfig {
            file_enabled: true,
            ..LoggingConfig::default()
        };

        assert_eq!(config.logging, expected);
    }

    #[test]
    fn logging_config_reads_each_source_path_name() {
        let names = ["full", "short", "file_name", "hidden"];
        let actual: Vec<SourcePathDisplay> = names
            .into_iter()
            .map(|name| parse_with_logging_section(&format!("source_path = \"{name}\"")))
            .map(|config| config.logging.source_path)
            .collect();
        let expected = vec![
            SourcePathDisplay::Full,
            SourcePathDisplay::Short,
            SourcePathDisplay::FileName,
            SourcePathDisplay::Hidden,
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn app_config_round_trips_the_logging_section() {
        let original = AppConfig {
            logging: LoggingConfig {
                level: "debug".to_string(),
                application_only: false,
                stdout: false,
                file_enabled: true,
                file_path: PathBuf::from("logs/app.log"),
                source_path: SourcePathDisplay::FileName,
            },
            ..AppConfig::default()
        };

        let text = toml::to_string_pretty(&original).expect("config must serialize");
        let parsed: AppConfig = toml::from_str(&text).expect("config must parse");

        assert_eq!(parsed.logging, original.logging);
    }

    #[test]
    fn logging_config_deserializes_global_bare_level_scope() {
        let text =
            format!("{DATABASE_SECTION}\n[logging]\nlevel = \"debug\"\napplication_only = false\n");
        let config: AppConfig = toml::from_str(&text).expect("config must parse");

        assert_eq!(config.logging.level, "debug");
        assert!(!config.logging.application_only);
    }

    #[test]
    fn logging_config_settings_include_the_file_only_when_enabled() {
        let disabled = LoggingConfig::default();
        let enabled = LoggingConfig {
            file_enabled: true,
            ..LoggingConfig::default()
        };
        let expected_path = enabled.file_path.as_path();

        assert_eq!(disabled.settings().file, None);
        assert_eq!(enabled.settings().file, Some(expected_path));
        assert!(disabled.settings().application_only);
    }
}
