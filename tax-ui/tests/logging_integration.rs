//! Initializes the real logging subscriber. Integration tests run in their own
//! process, so installing the global subscriber here cannot affect unit tests.
//!
//! This test expects `RUST_LOG` to be unset.

use pretty_assertions::assert_eq;
use std::fs;
use tax_ui::logging::{
    LogSettings, SourcePathDisplay, apply_settings, default_level, init_default_logging,
};

/// Message of the probe event written to the log file. It contains no spaces,
/// so it is the last space-separated word of its log line.
const PROBE_MESSAGE: &str = "integration-probe";

/// Message emitted below `warn` after switching a bare level to global scope.
const GLOBAL_PROBE_MESSAGE: &str = "global-integration-probe";

/// Message of the error returned by a second initialization.
const ALREADY_INSTALLED_MESSAGE: &str = "another logging subscriber is already installed";

#[test]
fn init_default_logging_installs_a_working_subscriber_once() {
    init_default_logging().expect("first initialization must succeed");

    let dir = tempfile::tempdir().expect("temporary directory must be created");
    // The `logs` directory does not exist yet; applying the settings creates it.
    let path = dir.path().join("logs").join("integration.log");
    let mut settings = LogSettings {
        level: default_level(),
        application_only: true,
        stdout: false,
        file: Some(path.as_path()),
        source_path: SourcePathDisplay::Full,
    };
    apply_settings(&settings).expect("settings must apply");

    // Under the default level, crates outside the workspace libraries (such as
    // this test crate) are shown at `warn` and above.
    tracing::info!("application-only-probe");
    tracing::warn!("{PROBE_MESSAGE}");

    settings.application_only = false;
    apply_settings(&settings).expect("global settings must apply");
    tracing::info!("{GLOBAL_PROBE_MESSAGE}");

    settings.file = None;
    apply_settings(&settings).expect("settings must apply");

    let contents = fs::read_to_string(&path).expect("log file must be readable");
    let last_words: Vec<&str> = contents
        .lines()
        .filter_map(|line| line.rsplit(' ').next())
        .collect();
    assert_eq!(last_words, vec![PROBE_MESSAGE, GLOBAL_PROBE_MESSAGE]);

    let second = init_default_logging().map_err(|error| error.to_string());
    assert_eq!(second, Err(ALREADY_INSTALLED_MESSAGE.to_string()));
}
