//! Runtime control of the installed logging subscriber.
//!
//! [`init`](super::init) builds a [`LoggingControl`] from the reload handles
//! and file slot of the subscriber it installs, then stores it with
//! [`install`]. The public control functions in the parent module reach it
//! through [`installed`].

use std::{path::Path, sync::OnceLock};
use thiserror::Error;
use tracing::Subscriber;
use tracing_subscriber::{EnvFilter, filter::LevelFilter, reload};

use super::{
    filter::{FilterError, LevelScope, LogFilterSource, parse_filter},
    format::{SourcePathDisplay, SourcePathHandle},
    writer::{FileSlot, OpenLogFileError, open_log_file},
};

/// Gate value that passes every event. The global level filter still decides
/// which events reach the gate.
pub(super) const GATE_OPEN: LevelFilter = LevelFilter::TRACE;

/// Gate value that blocks every event.
pub(super) const GATE_CLOSED: LevelFilter = LevelFilter::OFF;

/// Separates the descriptions of independent failures.
const FAILURE_SEPARATOR: &str = "; ";

/// Separates a message from the message of its cause.
const CAUSE_SEPARATOR: &str = ": ";

/// Controls for the subscriber installed by `init_default_logging`.
static CONTROL: OnceLock<LoggingControl> = OnceLock::new();

/// Replaces the global level filter of the installed subscriber.
type SetLevelFn = Box<dyn Fn(EnvFilter) -> Result<(), reload::Error> + Send + Sync>;

/// Replaces the value of an output gate of the installed subscriber.
type SetGateFn = Box<dyn Fn(LevelFilter) -> Result<(), reload::Error> + Send + Sync>;

/// Logging settings applied after the application configuration loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogSettings<'a> {
    /// Bare level (`error`, `warn`, `info`, `debug`, `trace`) or a full
    /// `EnvFilter` directive. Ignored while `RUST_LOG` sets the filter.
    pub level: &'a str,

    /// Whether a bare level applies only to the application workspace crates.
    /// Full filter directives define their own target scopes. Ignored while
    /// `RUST_LOG` sets the filter.
    pub application_only: bool,

    /// Whether log output is written to stdout.
    pub stdout: bool,

    /// File to write log output to, or `None` to disable file logging. A
    /// relative path is resolved against the current working directory, and
    /// missing parent directories are created.
    pub file: Option<&'a Path>,

    /// How the source location of each event is shown.
    pub source_path: SourcePathDisplay,
}

/// Error returned by runtime logging control.
#[derive(Debug, Error)]
pub(super) enum ControlError {
    /// Logging has not been initialized, or initialization did not install
    /// its subscriber.
    #[error("logging not yet initialized")]
    NotInitialized,

    /// A logging control was already installed in this process.
    #[error("logging control is already installed")]
    AlreadyInstalled,

    /// The requested filter could not be parsed.
    #[error(transparent)]
    Filter(#[from] FilterError),

    /// The log file could not be opened.
    #[error(transparent)]
    OpenFile(#[from] OpenLogFileError),

    /// The global level filter could not be replaced.
    #[error("cannot reload the log level filter")]
    ReloadLevel {
        #[source]
        source: reload::Error,
    },

    /// The stdout gate could not be replaced.
    #[error("cannot reload the stdout filter")]
    ReloadStdout {
        #[source]
        source: reload::Error,
    },

    /// The file gate could not be replaced.
    #[error("cannot reload the file filter")]
    ReloadFile {
        #[source]
        source: reload::Error,
    },
}

/// Error returned when one or more logging settings could not be applied.
#[derive(Debug, Error)]
#[error("{}", describe_failures(.failures))]
pub(super) struct SettingsError {
    failures: Vec<ControlError>,
}

/// Runtime controls for an installed subscriber.
///
/// The reload handles are stored as boxed closures so that the concrete type
/// of the layered subscriber does not become part of this type.
pub(super) struct LoggingControl {
    set_level: SetLevelFn,
    set_stdout_gate: SetGateFn,
    set_file_gate: SetGateFn,
    file_slot: FileSlot,
    source_path: SourcePathHandle,
    level_source: LogFilterSource,
}

impl LoggingControl {
    /// Builds controls from the reload handles, file slot, and source path
    /// handle of a subscriber, and the source of its startup filter.
    ///
    /// `L`, `O`, and `F` are the subscriber types below the global level
    /// filter, the stdout gate, and the file gate.
    pub(super) fn new<L, O, F>(
        level_handle: reload::Handle<EnvFilter, L>,
        stdout_handle: reload::Handle<LevelFilter, O>,
        file_handle: reload::Handle<LevelFilter, F>,
        file_slot: FileSlot,
        source_path: SourcePathHandle,
        level_source: LogFilterSource,
    ) -> Self
    where
        L: Subscriber + Send + Sync + 'static,
        O: Subscriber + Send + Sync + 'static,
        F: Subscriber + Send + Sync + 'static,
    {
        Self {
            set_level: Box::new(move |filter: EnvFilter| level_handle.reload(filter)),
            set_stdout_gate: Box::new(move |level: LevelFilter| stdout_handle.reload(level)),
            set_file_gate: Box::new(move |level: LevelFilter| file_handle.reload(level)),
            file_slot,
            source_path,
            level_source,
        }
    }

    /// Applies every setting independently, so one invalid setting does not
    /// prevent the others. The level and its scope are skipped while the
    /// startup filter comes from `RUST_LOG`, so the environment keeps
    /// precedence.
    pub(super) fn apply_settings(
        &self,
        settings: &LogSettings<'_>,
    ) -> Result<(), SettingsError> {
        let mut failures = Vec::new();

        if self.level_source == LogFilterSource::Default {
            let level_scope = if settings.application_only {
                LevelScope::ApplicationOnly
            } else {
                LevelScope::Global
            };
            failures.extend(self.set_log_level(settings.level, level_scope).err());
        }
        failures.extend(self.set_stdout_enabled(settings.stdout).err());
        self.set_source_path(settings.source_path);

        let file_result = match settings.file {
            Some(path) => self.enable_file_logging(path),
            None => self.disable_file_logging(),
        };
        failures.extend(file_result.err());

        if failures.is_empty() {
            Ok(())
        } else {
            Err(SettingsError { failures })
        }
    }

    /// Replaces the global level filter with the filter parsed from `input`.
    /// See [`parse_filter`] for the accepted input.
    pub(super) fn set_log_level(
        &self,
        input: &str,
        level_scope: LevelScope,
    ) -> Result<(), ControlError> {
        let filter = parse_filter(input, level_scope)?;
        (self.set_level)(filter).map_err(|source| ControlError::ReloadLevel { source })
    }

    /// Shows or hides stdout output without affecting file output.
    pub(super) fn set_stdout_enabled(
        &self,
        enabled: bool,
    ) -> Result<(), ControlError> {
        (self.set_stdout_gate)(gate_level(enabled))
            .map_err(|source| ControlError::ReloadStdout { source })
    }

    /// Changes how the source location of each event is shown, for every
    /// output at once.
    pub(super) fn set_source_path(
        &self,
        display: SourcePathDisplay,
    ) {
        self.source_path.set(display);
    }

    /// Starts writing log output to the file at `path`, replacing any file
    /// that was set before. Missing parent directories are created.
    pub(super) fn enable_file_logging(
        &self,
        path: &Path,
    ) -> Result<(), ControlError> {
        let file = open_log_file(path)?;
        self.file_slot.set_file(file);
        self.reload_file_gate(GATE_OPEN)
    }

    /// Stops writing log output to a file and closes the current file.
    pub(super) fn disable_file_logging(&self) -> Result<(), ControlError> {
        // Close the gate first so no event is formatted for a file that is
        // about to close. The file is closed even if the gate cannot change.
        let result = self.reload_file_gate(GATE_CLOSED);
        self.file_slot.clear();
        result
    }

    /// Replaces the file gate value.
    fn reload_file_gate(
        &self,
        level: LevelFilter,
    ) -> Result<(), ControlError> {
        (self.set_file_gate)(level).map_err(|source| ControlError::ReloadFile { source })
    }
}

/// Stores `control` as the process-wide logging control. Call only after the
/// subscriber that owns its handles has been installed.
pub(super) fn install(control: LoggingControl) -> Result<(), ControlError> {
    CONTROL
        .set(control)
        .map_err(|_| ControlError::AlreadyInstalled)
}

/// Returns the installed logging control.
pub(super) fn installed() -> Result<&'static LoggingControl, ControlError> {
    CONTROL.get().ok_or(ControlError::NotInitialized)
}

/// Returns the gate value for an output that is enabled or disabled.
fn gate_level(enabled: bool) -> LevelFilter {
    if enabled { GATE_OPEN } else { GATE_CLOSED }
}

/// Joins the description of each failure, including its causes.
fn describe_failures(failures: &[ControlError]) -> String {
    let mut description = String::new();
    for (index, failure) in failures.iter().enumerate() {
        if index > 0 {
            description.push_str(FAILURE_SEPARATOR);
        }
        description.push_str(&describe_with_causes(failure));
    }
    description
}

/// Returns the message of `error` followed by the messages of its causes.
fn describe_with_causes(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut cause = error.source();
    while let Some(current) = cause {
        message.push_str(CAUSE_SEPARATOR);
        message.push_str(&current.to_string());
        cause = current.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use tracing::{Dispatch, Level};
    use tracing_subscriber::{Layer, layer::SubscriberExt};

    use crate::logging::test_support::{
        PROBE_MESSAGE, RecordingLayer, line_count, recorded, temp_dir,
    };

    /// Bare level installed when each harness starts.
    const INITIAL_LEVEL: &str = "info";

    /// Bare level that lets `debug` events from the workspace crates through.
    const DEBUG_LEVEL: &str = "debug";

    /// Event target representing a crate outside the application workspace.
    const DEPENDENCY_TARGET: &str = "gpui";

    // --- Test helpers ---

    /// A scoped subscriber with the same layer structure as the one installed
    /// by `init`, with recording layers in place of the stdout formatter and
    /// next to the file formatter.
    ///
    /// Events emitted in these tests use their default target, the module
    /// path `tax_ui::logging::control::tests`, which the bare-level presets
    /// treat as a workspace crate target.
    struct Harness {
        control: LoggingControl,
        stdout: RecordingLayer,
        file_events: RecordingLayer,
        dispatch: Dispatch,
    }

    impl Harness {
        /// Builds a harness whose startup filter came from the built-in preset.
        fn new() -> Self {
            Self::with_source(LogFilterSource::Default)
        }

        /// Builds a harness whose startup filter is recorded as `source`.
        fn with_source(source: LogFilterSource) -> Self {
            let initial = parse_filter(INITIAL_LEVEL, LevelScope::ApplicationOnly)
                .expect("initial level must parse");
            let (level_filter, level_handle) = reload::Layer::new(initial);
            let (stdout_gate, stdout_handle) = reload::Layer::new(GATE_OPEN);
            let (file_gate, file_handle) = reload::Layer::new(GATE_CLOSED);
            let stdout = RecordingLayer::default();
            let file_events = RecordingLayer::default();
            let slot = FileSlot::default();

            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(slot.clone())
                .and_then(file_events.clone())
                .with_filter(file_gate);
            let subscriber = tracing_subscriber::registry()
                .with(level_filter)
                .with(stdout.clone().with_filter(stdout_gate))
                .with(file_layer);

            Self {
                control: LoggingControl::new(
                    level_handle,
                    stdout_handle,
                    file_handle,
                    slot,
                    SourcePathHandle::default(),
                    source,
                ),
                stdout,
                file_events,
                dispatch: Dispatch::new(subscriber),
            }
        }

        /// Runs `emit` with this harness as the current subscriber.
        fn run(
            &self,
            emit: impl FnOnce(),
        ) {
            tracing::dispatcher::with_default(&self.dispatch, emit);
        }
    }

    // --- Helper functions ---

    #[test]
    fn gate_level_opens_when_enabled_and_closes_when_disabled() {
        assert_eq!(
            [gate_level(true), gate_level(false)],
            [LevelFilter::TRACE, LevelFilter::OFF]
        );
    }

    #[test]
    fn describe_with_causes_appends_each_cause() {
        let error = anyhow::anyhow!("inner").context("outer");
        assert_eq!(describe_with_causes(&*error), "outer: inner");
    }

    #[test]
    fn settings_error_lists_every_failure() {
        let error = SettingsError {
            failures: vec![
                ControlError::Filter(FilterError::Empty),
                ControlError::NotInitialized,
            ],
        };

        assert_eq!(
            error.to_string(),
            "empty log filter; logging not yet initialized"
        );
    }

    // --- Level control ---

    #[test]
    fn set_log_level_replaces_the_global_filter() {
        let harness = Harness::new();
        let control = &harness.control;

        harness.run(|| {
            tracing::debug!("{PROBE_MESSAGE}");
            control
                .set_log_level(DEBUG_LEVEL, LevelScope::ApplicationOnly)
                .expect("apply level");
            tracing::debug!("{PROBE_MESSAGE}");
        });

        assert_eq!(
            harness.stdout.events(),
            vec![recorded(module_path!(), Level::DEBUG)]
        );
    }

    #[test]
    fn set_log_level_rejects_invalid_input_and_keeps_the_current_filter() {
        let harness = Harness::new();

        match harness
            .control
            .set_log_level("   ", LevelScope::ApplicationOnly)
            .unwrap_err()
        {
            ControlError::Filter(FilterError::Empty) => {}
            other => panic!("expected ControlError::Filter(FilterError::Empty), got {other:?}"),
        }

        harness.run(|| {
            tracing::debug!("{PROBE_MESSAGE}");
            tracing::info!("{PROBE_MESSAGE}");
        });

        assert_eq!(
            harness.stdout.events(),
            vec![recorded(module_path!(), Level::INFO)]
        );
    }

    // --- Output control ---

    #[test]
    fn set_stdout_enabled_gates_stdout_without_affecting_the_file() {
        let dir = temp_dir();
        let path = dir.path().join("gate.log");
        let harness = Harness::new();
        let control = &harness.control;
        control.enable_file_logging(&path).expect("open log file");

        harness.run(|| {
            tracing::info!("{PROBE_MESSAGE}");
            control.set_stdout_enabled(false).expect("close stdout");
            tracing::warn!("{PROBE_MESSAGE}");
            control.set_stdout_enabled(true).expect("open stdout");
            tracing::error!("{PROBE_MESSAGE}");
        });
        control.disable_file_logging().expect("close log file");

        let target = module_path!();
        let expected = vec![
            recorded(target, Level::INFO),
            recorded(target, Level::ERROR),
        ];

        assert_eq!(harness.stdout.events(), expected);
        assert_eq!(line_count(&path), 3);
    }

    #[test]
    fn file_logging_reaches_the_file_only_while_enabled() {
        let dir = temp_dir();
        let path = dir.path().join("toggle.log");
        let harness = Harness::new();
        let control = &harness.control;

        harness.run(|| {
            tracing::info!("{PROBE_MESSAGE}");
            control.enable_file_logging(&path).expect("open log file");
            tracing::warn!("{PROBE_MESSAGE}");
            control.disable_file_logging().expect("close log file");
            tracing::error!("{PROBE_MESSAGE}");
        });

        // The file gate starts closed and closes again when file logging is
        // disabled, so only the event emitted in between reaches the file side.
        assert_eq!(
            harness.file_events.events(),
            vec![recorded(module_path!(), Level::WARN)]
        );
        assert_eq!(harness.stdout.events().len(), 3);
        assert_eq!(line_count(&path), 1);
    }

    #[test]
    fn enable_file_logging_reports_open_failure_and_keeps_the_file_gate_closed() {
        let dir = temp_dir();
        // The path is an existing directory, so it cannot be opened as a file.
        let path = dir.path();
        let harness = Harness::new();

        match harness.control.enable_file_logging(path).unwrap_err() {
            ControlError::OpenFile(_) => {}
            other => panic!("expected ControlError::OpenFile, got {other:?}"),
        }

        harness.run(|| tracing::info!("{PROBE_MESSAGE}"));

        assert_eq!(harness.file_events.events().len(), 0);
    }

    // --- Settings ---

    #[test]
    fn apply_settings_applies_every_setting() {
        let dir = temp_dir();
        let path = dir.path().join("settings.log");
        let harness = Harness::new();
        let control = &harness.control;
        let settings = LogSettings {
            level: DEBUG_LEVEL,
            application_only: true,
            stdout: false,
            file: Some(path.as_path()),
            source_path: SourcePathDisplay::FileName,
        };

        control.apply_settings(&settings).expect("apply");
        harness.run(|| tracing::debug!("{PROBE_MESSAGE}"));
        control.disable_file_logging().expect("close log file");

        assert_eq!(control.source_path.get(), SourcePathDisplay::FileName);
        assert_eq!(harness.stdout.events().len(), 0);
        assert_eq!(
            harness.file_events.events(),
            vec![recorded(module_path!(), Level::DEBUG)]
        );
        assert_eq!(line_count(&path), 1);
    }

    #[test]
    fn apply_settings_can_apply_a_bare_level_globally() {
        let harness = Harness::new();
        let settings = LogSettings {
            level: DEBUG_LEVEL,
            application_only: false,
            stdout: true,
            file: None,
            source_path: SourcePathDisplay::Full,
        };

        harness.control.apply_settings(&settings).expect("apply");
        harness.run(|| {
            tracing::debug!(target: DEPENDENCY_TARGET, "{PROBE_MESSAGE}");
        });

        assert_eq!(
            harness.stdout.events(),
            vec![recorded(DEPENDENCY_TARGET, Level::DEBUG)]
        );
    }

    #[test]
    fn apply_settings_keeps_the_environment_level() {
        let harness = Harness::with_source(LogFilterSource::Environment);
        let control = &harness.control;
        let settings = LogSettings {
            level: DEBUG_LEVEL,
            application_only: false,
            stdout: true,
            file: None,
            source_path: SourcePathDisplay::Full,
        };

        control.apply_settings(&settings).expect("apply");
        harness.run(|| {
            tracing::debug!("{PROBE_MESSAGE}");
            tracing::info!("{PROBE_MESSAGE}");
        });

        assert_eq!(
            harness.stdout.events(),
            vec![recorded(module_path!(), Level::INFO)]
        );
    }

    #[test]
    fn apply_settings_reports_each_failure_and_applies_the_rest() {
        let dir = temp_dir();
        let harness = Harness::new();
        let settings = LogSettings {
            level: "   ",
            application_only: true,
            stdout: false,
            // The path is an existing directory, so it cannot be opened as a file.
            file: Some(dir.path()),
            source_path: SourcePathDisplay::Hidden,
        };

        let error = harness.control.apply_settings(&settings).unwrap_err();
        match error.failures.as_slice() {
            [
                ControlError::Filter(FilterError::Empty),
                ControlError::OpenFile(_),
            ] => {}
            other => panic!("expected empty filter and open file failures, got {other:?}"),
        }

        harness.run(|| tracing::info!("{PROBE_MESSAGE}"));

        assert_eq!(harness.stdout.events().len(), 0);
        assert_eq!(harness.control.source_path.get(), SourcePathDisplay::Hidden);
    }

    #[test]
    fn apply_settings_without_a_file_disables_file_logging() {
        let dir = temp_dir();
        let path = dir.path().join("previous.log");
        let harness = Harness::new();
        let control = &harness.control;
        control.enable_file_logging(&path).expect("open log file");
        let settings = LogSettings {
            level: INITIAL_LEVEL,
            application_only: true,
            stdout: true,
            file: None,
            source_path: SourcePathDisplay::Full,
        };

        control.apply_settings(&settings).expect("apply");
        harness.run(|| tracing::info!("{PROBE_MESSAGE}"));

        assert_eq!(harness.file_events.events().len(), 0);
        assert_eq!(line_count(&path), 0);
    }
}
