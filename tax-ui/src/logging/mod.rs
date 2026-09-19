//! Application logging.
//!
//! - [`init_default_logging`] installs the subscriber once at startup.
//! - [`apply_settings`] applies [`LogSettings`] from the application
//!   configuration once it has loaded.
//! - [`set_log_level`], [`set_stdout_enabled`], [`set_source_path`],
//!   [`enable_file_logging`], and [`disable_file_logging`] change individual
//!   settings at runtime.
//! - [`log_task_error`] reports failed background tasks.

mod control;
mod filter;
mod format;
mod init;
mod report;
#[cfg(test)]
mod test_support;
mod writer;

pub use self::{control::LogSettings, format::SourcePathDisplay, report::log_task_error};

use anyhow::Result;
use std::path::Path;

// --- Public API ---

/// Initializes logging. Call once at startup.
///
/// - Stdout: colored when attached to a terminal, plain when piped.
/// - File: inactive until [`apply_settings`] or [`enable_file_logging`]
///   enables it.
/// - Level: if `RUST_LOG` is unset, uses a workspace-only default (`warn` for
///   dependencies, [`default_level`] for the workspace crates) until
///   [`apply_settings`] sets the configured level. Set `RUST_LOG` to override
///   both (e.g. `RUST_LOG=gpui=debug,tax_ui=trace`). An invalid value is
///   reported as a warning, and the default is used.
///
/// # Errors
///
/// Returns an error if logging could not be fully set up. The application can
/// continue: depending on the cause, another subscriber may be receiving
/// events, or this subscriber may be active without capturing records from
/// the `log` crate.
pub fn init_default_logging() -> Result<()> {
    init::install_default_subscriber()?;
    Ok(())
}

/// Applies logging settings from the application configuration.
///
/// Each setting is applied independently, so an invalid level does not
/// prevent file logging from starting. While `RUST_LOG` is set, `level` and
/// its application-only scope are ignored so the environment keeps
/// precedence.
///
/// # Errors
///
/// Returns an error that describes every setting that could not be applied,
/// or an error if logging is not initialized.
pub fn apply_settings(settings: &LogSettings<'_>) -> Result<()> {
    control::installed()?.apply_settings(settings)?;
    Ok(())
}

/// Returns the bare level used when neither the configuration nor `RUST_LOG`
/// sets one.
pub fn default_level() -> &'static str {
    filter::default_level_name()
}

/// Changes the active log filter at runtime.
///
/// A **bare** level (`error`, `warn`, `info`, `debug`, `trace`, case-insensitive)
/// applies that level only to the workspace crates, with a global default of
/// `warn` so dependency crates stay quiet.
///
/// Any other string is parsed as a full
/// [`EnvFilter`](tracing_subscriber::EnvFilter) directive (e.g.
/// `gpui=debug,tax_ui=trace`) for advanced use.
pub fn set_log_level(level: &str) -> Result<()> {
    control::installed()?.set_log_level(level, filter::LevelScope::ApplicationOnly)?;
    Ok(())
}

/// Shows or hides stdout log output without affecting file logging.
pub fn set_stdout_enabled(enabled: bool) -> Result<()> {
    control::installed()?.set_stdout_enabled(enabled)?;
    Ok(())
}

/// Changes how the source location of each event is shown, for stdout and
/// file output at once.
pub fn set_source_path(display: SourcePathDisplay) -> Result<()> {
    control::installed()?.set_source_path(display);
    Ok(())
}

/// Starts writing log output to `path`. Safe to call after initialization.
/// If a file is already open it is replaced.
/// Missing parent directories are created.
/// If logging is not initialized, returns an error without creating the file.
pub fn enable_file_logging(path: &Path) -> Result<()> {
    control::installed()?.enable_file_logging(path)?;
    Ok(())
}

/// Closes the current log file. Subsequent records go to stdout only
/// (if stdout is enabled).
pub fn disable_file_logging() -> Result<()> {
    control::installed()?.disable_file_logging()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::fs;

    use crate::logging::{control::ControlError, test_support::temp_dir};

    // No unit test in this crate installs the global subscriber, so the public
    // control functions always see an uninitialized state here.

    /// Panics unless `error` wraps [`ControlError::NotInitialized`].
    fn expect_not_initialized(error: &anyhow::Error) {
        match error.downcast_ref::<ControlError>() {
            Some(ControlError::NotInitialized) => {}
            other => panic!("expected ControlError::NotInitialized, got {other:?}"),
        }
    }

    #[test]
    fn apply_settings_before_init_returns_not_initialized() {
        let settings = LogSettings {
            level: default_level(),
            application_only: true,
            stdout: true,
            file: None,
            source_path: SourcePathDisplay::Full,
        };

        let error = apply_settings(&settings).unwrap_err();
        expect_not_initialized(&error);
    }

    #[test]
    fn set_log_level_before_init_returns_not_initialized() {
        let error = set_log_level("info").unwrap_err();
        expect_not_initialized(&error);
    }

    #[test]
    fn set_stdout_enabled_before_init_returns_not_initialized() {
        let error = set_stdout_enabled(false).unwrap_err();
        expect_not_initialized(&error);
    }

    #[test]
    fn set_source_path_before_init_returns_not_initialized() {
        let error = set_source_path(SourcePathDisplay::Hidden).unwrap_err();
        expect_not_initialized(&error);
    }

    #[test]
    fn enable_file_logging_before_init_returns_not_initialized_without_creating_a_file() {
        let dir = temp_dir();
        let path = dir.path().join("app.log");

        let error = enable_file_logging(&path).unwrap_err();

        expect_not_initialized(&error);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn disable_file_logging_before_init_returns_not_initialized() {
        let error = disable_file_logging().unwrap_err();
        expect_not_initialized(&error);
    }
}
