//! Subscriber assembly and installation.

use std::io::{self, IsTerminal};
use thiserror::Error;
use tracing_subscriber::{
    Layer,
    layer::SubscriberExt,
    reload,
    util::{SubscriberInitExt, TryInitError},
};

use super::{
    control::{self, ControlError, GATE_CLOSED, GATE_OPEN, LoggingControl},
    filter::{LOG_FILTER_ENV_VAR, initial_filter},
    format::{LocalFmt, SourcePathHandle},
    writer::FileSlot,
};

/// Error returned when logging cannot be fully set up.
#[derive(Debug, Error)]
pub(super) enum InitError {
    /// Another global subscriber was already installed. This application's
    /// subscriber and its runtime controls are not active; events go to the
    /// other subscriber.
    #[error("another logging subscriber is already installed")]
    SubscriberAlreadySet {
        #[source]
        source: TryInitError,
    },

    /// This application's subscriber and its runtime controls are active, but
    /// records from the `log` crate are not forwarded to it because another
    /// `log` logger was already set.
    #[error("logging is active, but `log` crate records are not captured")]
    LogBridgeUnavailable {
        #[source]
        source: TryInitError,
    },

    /// The subscriber was installed, but its runtime controls could not be
    /// stored.
    #[error(transparent)]
    Control(#[from] ControlError),
}

/// Builds the application subscriber, installs it as the global default, and
/// stores its runtime controls.
pub(super) fn install_default_subscriber() -> Result<(), InitError> {
    let initial = initial_filter();
    let file_slot = FileSlot::default();
    let source_path = SourcePathHandle::default();

    // Global level filter: the ceiling for every output.
    let (level_filter, level_handle) = reload::Layer::new(initial.filter);
    // Per-output gates below the global filter. The file gate stays closed
    // until a file is set, so events are not formatted for an empty slot.
    let (stdout_gate, stdout_handle) = reload::Layer::new(GATE_OPEN);
    let (file_gate, file_handle) = reload::Layer::new(GATE_CLOSED);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .event_format(LocalFmt::new(source_path.clone()))
        .with_ansi(io::stdout().is_terminal())
        .with_filter(stdout_gate);

    let file_layer = tracing_subscriber::fmt::layer()
        .event_format(LocalFmt::new(source_path.clone()))
        .with_ansi(false)
        .with_writer(file_slot.clone())
        .with_filter(file_gate);

    let init_result = tracing_subscriber::registry()
        .with(level_filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init();

    // A reload handle can reach its layer only while the subscriber that owns
    // the layer exists. After a failed `try_init`, this shows whether the
    // subscriber was installed anyway, for example when only the `log` bridge
    // could not be set.
    let installed = level_handle.with_current(|_| ()).is_ok();

    if installed {
        let control = LoggingControl::new(
            level_handle,
            stdout_handle,
            file_handle,
            file_slot,
            source_path,
            initial.source,
        );
        control::install(control)?;

        if let Some(error) = initial.rejected_env {
            tracing::warn!(
                variable = LOG_FILTER_ENV_VAR,
                %error,
                "ignored invalid log filter variable"
            );
        }
    }

    init_result.map_err(|source| {
        if installed {
            InitError::LogBridgeUnavailable { source }
        } else {
            InitError::SubscriberAlreadySet { source }
        }
    })
}
