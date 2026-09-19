//! Helpers shared by the logging submodule tests.

use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
};
use tempfile::TempDir;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{Layer, layer::Context};

/// Message used by probe events whose text is not inspected.
pub(super) const PROBE_MESSAGE: &str = "probe";

/// Creates a temporary directory that is removed when dropped.
pub(super) fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("temporary directory must be created")
}

/// Reads the whole file at `path` as UTF-8 text.
pub(super) fn read_log(path: &Path) -> String {
    fs::read_to_string(path).expect("log file must be readable")
}

/// Returns the number of lines in the file at `path`.
pub(super) fn line_count(path: &Path) -> usize {
    read_log(path).lines().count()
}

/// Layer that records the target and level of every event that reaches it.
#[derive(Clone, Default)]
pub(super) struct RecordingLayer(Arc<Mutex<Vec<(String, Level)>>>);

impl RecordingLayer {
    /// Returns the recorded events in the order they were received.
    pub(super) fn events(&self) -> Vec<(String, Level)> {
        self.0.lock().unwrap().clone()
    }
}

impl<S> Layer<S> for RecordingLayer
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &Event<'_>,
        _ctx: Context<'_, S>,
    ) {
        let meta = event.metadata();
        self.0
            .lock()
            .unwrap()
            .push((meta.target().to_string(), *meta.level()));
    }
}

/// Builds an expected recorded event.
pub(super) fn recorded(
    target: &str,
    level: Level,
) -> (String, Level) {
    (target.to_string(), level)
}
