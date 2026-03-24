mod store;

pub use store::{ConfigStore, TomlConfigStore};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

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
// AppConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub database_backend: DatabaseBackend,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: "taxes.db".into(),
            database_backend: DatabaseBackend::Sqlite,
        }
    }
}

impl Global for AppConfig {}

/// Opaque wrapper so a `dyn ConfigStore` can live in gpui's global map.
struct ConfigStoreHandle(Box<dyn ConfigStore>);
impl Global for ConfigStoreHandle {}

impl AppConfig {
    /// Load (or create) config via `store` and install both the config
    /// and the store as gpui globals.
    pub fn init(
        cx: &mut App,
        store: impl ConfigStore,
    ) -> anyhow::Result<()> {
        let config = store.load_or_init()?;
        cx.set_global(config);
        cx.set_global(ConfigStoreHandle(Box::new(store)));
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
