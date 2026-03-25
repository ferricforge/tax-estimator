use super::AppConfig;
use anyhow::Context as _;
use std::path::{Path, PathBuf};

/// Backend-agnostic configuration persistence.
///
/// Current implementation: [`TomlConfigStore`].
/// Planned: Windows Registry, macOS plist.
pub trait ConfigStore: Send + Sync + 'static {
    /// Does a persisted config already exist?
    fn exists(&self) -> bool;

    /// Load an existing config. Errors if missing or malformed.
    fn load(&self) -> anyhow::Result<AppConfig>;

    /// Persist the given config.
    fn save(
        &self,
        config: &AppConfig,
    ) -> anyhow::Result<()>;

    /// Load if present; otherwise write defaults and return them.
    fn load_or_init(&self) -> anyhow::Result<AppConfig> {
        if self.exists() {
            self.load()
        } else {
            tracing::info!("No existing config found; writing defaults");
            let cfg = AppConfig::default();
            self.save(&cfg)?;
            Ok(cfg)
        }
    }
}

// ---------------------------------------------------------------------------
// TOML file backend
// ---------------------------------------------------------------------------

pub struct TomlConfigStore {
    path: PathBuf,
}

impl TomlConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Platform-appropriate default location:
    /// * Windows: `%APPDATA%\TaxEstimator\config.toml`
    /// * macOS:   `~/Library/Application Support/TaxEstimator/config.toml`
    /// * Linux:   `$XDG_CONFIG_HOME/TaxEstimator/config.toml`
    ///   (falls back to `~/.config/…`)
    pub fn default_location() -> anyhow::Result<Self> {
        Ok(Self::new(default_config_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ConfigStore for TomlConfigStore {
    fn exists(&self) -> bool {
        self.path.is_file()
    }

    fn load(&self) -> anyhow::Result<AppConfig> {
        let text = std::fs::read_to_string(&self.path)
            .with_context(|| format!("reading config file {}", self.path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("parsing config file {}", self.path.display()))
    }

    fn save(
        &self,
        config: &AppConfig,
    ) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(config)?;
        std::fs::write(&self.path, text)
            .with_context(|| format!("writing config file {}", self.path.display()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Platform config paths
// ---------------------------------------------------------------------------

const APP_DIR: &str = "TaxEstimator";
const FILE_NAME: &str = "config.toml";

#[cfg(target_os = "windows")]
fn default_config_path() -> anyhow::Result<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .context("%APPDATA% not set")?;
    Ok(base.join(APP_DIR).join(FILE_NAME))
}

#[cfg(target_os = "macos")]
fn default_config_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("$HOME not set")?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join(APP_DIR)
        .join(FILE_NAME))
}

#[cfg(target_os = "linux")]
fn default_config_path() -> anyhow::Result<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .context("neither $XDG_CONFIG_HOME nor $HOME set")?;
    Ok(base.join(APP_DIR).join(FILE_NAME))
}
