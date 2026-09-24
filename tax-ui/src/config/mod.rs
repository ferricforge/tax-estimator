mod database;
mod recent;
mod store;

pub use database::{DatabaseBackend, DatabaseConfig, PoolSettings};
pub use recent::{RecentConfig, RecentConnection};
pub use store::{ConfigStore, TomlConfigStore};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::logging::{self, LogSettings, SourcePathDisplay};

/// Application name used for the configuration directory and the default log
/// file name.
const APP_NAME: &str = "TaxEstimator";

/// File extension of the default log file.
const LOG_FILE_EXTENSION: &str = "log";

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

/// The whole configuration: one field per section of the file.
///
/// Every section is optional in the file; a missing section uses its
/// defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "AppConfigFile")]
pub struct AppConfig {
    /// Database settings, stored under `[database]`.
    pub database: DatabaseConfig,

    /// Logging settings, stored under `[logging]`.
    pub logging: LoggingConfig,

    /// Recently used connections, stored under `[recent]`.
    pub recent: RecentConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            logging: LoggingConfig::default(),
            recent: RecentConfig::default(),
        }
    }
}

/// The configuration as it may appear on disk.
///
/// Besides the current sections, this accepts the two top-level keys that
/// held the database settings before the `[database]` section existed, so an
/// older file keeps pointing at the same database. The former keys are read
/// only when `[database]` is absent. They are never written: the next save
/// produces the current layout.
#[derive(Deserialize)]
struct AppConfigFile {
    database: Option<DatabaseConfig>,

    #[serde(default)]
    logging: LoggingConfig,

    #[serde(default)]
    recent: RecentConfig,

    /// Former name of `database.url`.
    database_url: Option<String>,

    /// Former name of `database.backend`.
    database_backend: Option<DatabaseBackend>,
}

impl From<AppConfigFile> for AppConfig {
    fn from(file: AppConfigFile) -> Self {
        let database = match file.database {
            Some(database) => database,
            None => {
                let defaults = DatabaseConfig::default();
                DatabaseConfig {
                    backend: file.database_backend.unwrap_or(defaults.backend),
                    url: file.database_url.unwrap_or(defaults.url),
                    pool: defaults.pool,
                }
            }
        };

        Self {
            database,
            logging: file.logging,
            recent: file.recent,
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

    /// A `[database]` section with the default values.
    const DATABASE_SECTION: &str = r#"
[database]
backend = "sqlite"
url = "taxes.db"
"#;

    /// Top-level database keys used before the `[database]` section existed.
    const FORMER_DATABASE_KEYS: &str = r#"
database_url = "old.db"
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
    fn empty_file_uses_defaults_for_every_section() {
        let config: AppConfig = toml::from_str("").expect("config must parse");

        assert_eq!(config.database, DatabaseConfig::default());
        assert_eq!(config.logging, LoggingConfig::default());
        assert_eq!(config.recent, RecentConfig::default());
    }

    #[test]
    fn app_config_reads_the_recent_section() {
        let text = r#"
[recent]
limit = 5

[[recent.connections]]
backend = "sqlite"
url = "/data/2024.db"

[[recent.connections]]
url = "/data/2023.db"
"#;
        let config: AppConfig = toml::from_str(text).expect("config must parse");
        let expected = RecentConfig {
            limit: 5,
            connections: vec![
                RecentConnection {
                    backend: DatabaseBackend::Sqlite,
                    url: "/data/2024.db".to_string(),
                },
                RecentConnection {
                    backend: DatabaseBackend::Sqlite,
                    url: "/data/2023.db".to_string(),
                },
            ],
        };

        assert_eq!(config.recent, expected);
    }

    #[test]
    fn app_config_round_trips_the_recent_section() {
        let original = AppConfig {
            recent: RecentConfig {
                limit: 3,
                connections: vec![
                    RecentConnection {
                        backend: DatabaseBackend::Sqlite,
                        url: "/data/2024.db".to_string(),
                    },
                    RecentConnection {
                        backend: DatabaseBackend::Sqlite,
                        url: "/data/2023.db".to_string(),
                    },
                ],
            },
            ..AppConfig::default()
        };

        let text = toml::to_string_pretty(&original).expect("config must serialize");
        let parsed: AppConfig = toml::from_str(&text).expect("config must parse");

        assert_eq!(parsed.recent, original.recent);
    }

    #[test]
    fn app_config_reads_the_database_section() {
        let text = r#"
[database]
url = "other.db"

[database.pool]
max_connections = 3
"#;
        let config: AppConfig = toml::from_str(text).expect("config must parse");
        let expected = DatabaseConfig {
            url: "other.db".to_string(),
            pool: PoolSettings {
                max_connections: 3,
                ..PoolSettings::default()
            },
            ..DatabaseConfig::default()
        };

        assert_eq!(config.database, expected);
    }

    #[test]
    fn app_config_round_trips_the_database_section() {
        let original = AppConfig {
            database: DatabaseConfig {
                backend: DatabaseBackend::Sqlite,
                url: "projects/2025.db".to_string(),
                pool: PoolSettings {
                    max_connections: 3,
                    idle_timeout_secs: 0,
                    ..PoolSettings::default()
                },
            },
            ..AppConfig::default()
        };

        let text = toml::to_string_pretty(&original).expect("config must serialize");
        let parsed: AppConfig = toml::from_str(&text).expect("config must parse");

        assert_eq!(parsed.database, original.database);
    }

    #[test]
    fn app_config_reads_the_former_top_level_database_keys() {
        let config: AppConfig = toml::from_str(FORMER_DATABASE_KEYS).expect("config must parse");

        assert_eq!(config.database.url, "old.db");
        assert_eq!(config.database.backend, DatabaseBackend::Sqlite);
        assert_eq!(config.database.pool, PoolSettings::default());
    }

    #[test]
    fn database_section_takes_precedence_over_the_former_keys() {
        let text = format!("{FORMER_DATABASE_KEYS}\n[database]\nurl = \"new.db\"\n");
        let config: AppConfig = toml::from_str(&text).expect("config must parse");

        assert_eq!(config.database.url, "new.db");
    }

    #[test]
    fn saved_config_never_contains_the_former_keys() {
        let text = toml::to_string_pretty(&AppConfig::default()).expect("config must serialize");

        assert!(!text.contains("database_url"));
        assert!(!text.contains("database_backend"));
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
