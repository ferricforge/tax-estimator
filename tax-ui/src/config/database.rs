//! The `[database]` section of the configuration file.

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tax_core::db::PoolConfig;

/// Database location used when the configuration file does not name one.
const DEFAULT_URL: &str = "taxes.db";

/// SQLite location that names a private in-memory database.
pub(super) const SQLITE_IN_MEMORY_URL: &str = ":memory:";

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

    /// Whether `url` names a database that can be opened without creating
    /// it. A SQLite in-memory location counts as existing, because there is
    /// nothing to find.
    pub fn location_exists(
        &self,
        url: &str,
    ) -> bool {
        match self {
            Self::Sqlite => url == SQLITE_IN_MEMORY_URL || Path::new(url).is_file(),
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
// DatabaseConfig
// ---------------------------------------------------------------------------

/// Database settings stored in the `[database]` section of the configuration.
///
/// The section and each of its fields are optional; missing values use the
/// defaults.
///
/// ```toml
/// [database]
/// backend = "sqlite"
/// url = "taxes.db"
///
/// [database.pool]
/// max_connections = 10
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// Backend that opens [`url`](Self::url).
    pub backend: DatabaseBackend,

    /// Backend-specific location. For SQLite: a file path or `":memory:"`.
    /// A relative path is resolved against the current working directory.
    pub url: String,

    /// Connection pool settings, stored under `[database.pool]`.
    pub pool: PoolSettings,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: DatabaseBackend::default(),
            url: DEFAULT_URL.to_string(),
            pool: PoolSettings::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// PoolSettings
// ---------------------------------------------------------------------------

/// Connection pool settings stored in the `[database.pool]` section.
///
/// This is the file representation of [`PoolConfig`]. Durations are whole
/// seconds. TOML has no null value, so `0` stands for "no limit" in
/// `idle_timeout_secs` and `max_lifetime_secs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PoolSettings {
    /// Most connections open at once. Must be at least 1.
    pub max_connections: u32,

    /// Connections kept open while idle. Cannot exceed `max_connections`.
    pub min_connections: u32,

    /// Longest wait for a free connection, in seconds. Must be greater than 0.
    pub acquire_timeout_secs: u64,

    /// Idle seconds after which a connection is closed. `0` means never.
    pub idle_timeout_secs: u64,

    /// Seconds after which a connection is closed and replaced. `0` means never.
    pub max_lifetime_secs: u64,

    /// Whether a connection is checked before it is handed out.
    pub test_before_acquire: bool,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self::from(&PoolConfig::default())
    }
}

impl From<&PoolConfig> for PoolSettings {
    fn from(pool: &PoolConfig) -> Self {
        Self {
            max_connections: pool.max_connections,
            min_connections: pool.min_connections,
            acquire_timeout_secs: pool.acquire_timeout.as_secs(),
            idle_timeout_secs: optional_secs(pool.idle_timeout),
            max_lifetime_secs: optional_secs(pool.max_lifetime),
            test_before_acquire: pool.test_before_acquire,
        }
    }
}

impl PoolSettings {
    /// Converts the file representation into the typed pool configuration.
    pub fn to_pool_config(&self) -> PoolConfig {
        PoolConfig {
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            acquire_timeout: Duration::from_secs(self.acquire_timeout_secs),
            idle_timeout: optional_duration(self.idle_timeout_secs),
            max_lifetime: optional_duration(self.max_lifetime_secs),
            test_before_acquire: self.test_before_acquire,
        }
    }
}

/// Reads a seconds value in which `0` means "no limit".
fn optional_duration(secs: u64) -> Option<Duration> {
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// Writes an optional duration as seconds, using `0` for "no limit".
fn optional_secs(duration: Option<Duration>) -> u64 {
    duration.map_or(0, |value| value.as_secs())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tax_core::db::PoolConfig;

    use super::{DatabaseBackend, DatabaseConfig, PoolSettings};

    fn parse(text: &str) -> DatabaseConfig {
        toml::from_str(text).expect("database section should parse")
    }

    #[test]
    fn missing_keys_use_defaults() {
        let parsed = parse("url = \"custom.db\"");

        assert_eq!(parsed.url, "custom.db");
        assert_eq!(parsed.backend, DatabaseBackend::Sqlite);
        assert_eq!(parsed.pool, PoolSettings::default());
    }

    #[test]
    fn partial_pool_table_keeps_other_defaults() {
        let parsed = parse("[pool]\nmax_connections = 4\n");
        let expected = PoolSettings {
            max_connections: 4,
            ..PoolSettings::default()
        };

        assert_eq!(parsed.pool, expected);
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let original = DatabaseConfig::default();
        let text = toml::to_string(&original).expect("defaults should serialize");

        assert_eq!(parse(&text), original);
    }

    #[test]
    fn default_settings_convert_to_default_pool_config() {
        assert_eq!(
            PoolSettings::default().to_pool_config(),
            PoolConfig::default()
        );
    }

    #[test]
    fn zero_seconds_disables_optional_timeouts() {
        let parsed = parse("[pool]\nidle_timeout_secs = 0\nmax_lifetime_secs = 0\n");
        let pool = parsed.pool.to_pool_config();

        assert_eq!(pool.idle_timeout, None);
        assert_eq!(pool.max_lifetime, None);
    }

    #[test]
    fn unknown_backend_is_rejected() {
        let result: Result<DatabaseConfig, _> = toml::from_str("backend = \"oracle\"");

        assert!(result.is_err());
    }

    #[test]
    fn sqlite_location_exists_only_for_files_and_memory() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let present = dir.path().join("present.db");
        std::fs::write(&present, b"").expect("file should be written");
        let absent = dir.path().join("absent.db");

        assert!(DatabaseBackend::Sqlite.location_exists(":memory:"));
        assert!(DatabaseBackend::Sqlite.location_exists(&present.to_string_lossy()));
        assert!(!DatabaseBackend::Sqlite.location_exists(&absent.to_string_lossy()));
        assert!(!DatabaseBackend::Sqlite.location_exists(""));
    }
}
