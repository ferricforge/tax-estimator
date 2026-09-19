//! Reporting failures from background tasks.

use anyhow::Result;
use tracing::error;

/// Logs a background task failure with context. Does nothing when `result`
/// is `Ok`.
pub fn log_task_error(
    task_name: &'static str,
    result: Result<()>,
) {
    if let Err(error) = result {
        error!(task = task_name, ?error, "background task failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::{
        collections::BTreeMap,
        fmt,
        sync::{Arc, Mutex},
    };
    use tracing::{Event, Level, Subscriber};
    use tracing_subscriber::{
        Layer,
        layer::{Context, SubscriberExt},
    };

    use crate::logging::test_support::RecordingLayer;

    /// Task name used by the probe calls.
    const PROBE_TASK: &str = "probe_task";

    // --- Test helpers ---

    /// Layer that records the fields of every event as text, keyed by name.
    #[derive(Clone, Default)]
    struct FieldRecorder(Arc<Mutex<Vec<BTreeMap<String, String>>>>);

    impl FieldRecorder {
        fn events(&self) -> Vec<BTreeMap<String, String>> {
            self.0.lock().unwrap().clone()
        }
    }

    impl<S> Layer<S> for FieldRecorder
    where
        S: Subscriber,
    {
        fn on_event(
            &self,
            event: &Event<'_>,
            _ctx: Context<'_, S>,
        ) {
            let mut fields = FieldMap::default();
            event.record(&mut fields);
            self.0.lock().unwrap().push(fields.0);
        }
    }

    /// Collects field values as text. String values are stored without quotes.
    #[derive(Default)]
    struct FieldMap(BTreeMap<String, String>);

    impl tracing::field::Visit for FieldMap {
        fn record_str(
            &mut self,
            field: &tracing::field::Field,
            value: &str,
        ) {
            let name = field.name().to_string();
            self.0.insert(name, value.to_string());
        }

        fn record_debug(
            &mut self,
            field: &tracing::field::Field,
            value: &dyn fmt::Debug,
        ) {
            let name = field.name().to_string();
            self.0.insert(name, format!("{value:?}"));
        }
    }

    // --- log_task_error ---

    #[test]
    fn log_task_error_logs_an_error_event_with_the_task_name() {
        let levels = RecordingLayer::default();
        let fields = FieldRecorder::default();
        let subscriber = tracing_subscriber::registry()
            .with(levels.clone())
            .with(fields.clone());

        tracing::subscriber::with_default(subscriber, || {
            log_task_error(PROBE_TASK, Err(anyhow::anyhow!("probe failure")));
        });

        let actual_levels: Vec<Level> = levels
            .events()
            .into_iter()
            .map(|(_, level)| level)
            .collect();
        assert_eq!(actual_levels, vec![Level::ERROR]);

        let recorded_fields = fields.events();
        assert_eq!(recorded_fields.len(), 1);

        let event = &recorded_fields[0];
        let names: Vec<&str> = event.keys().map(String::as_str).collect();
        assert_eq!(names, vec!["error", "message", "task"]);
        assert_eq!(event["task"], PROBE_TASK);
        assert_eq!(event["message"], "background task failed");
    }

    #[test]
    fn log_task_error_logs_nothing_for_success() {
        let levels = RecordingLayer::default();
        let subscriber = tracing_subscriber::registry().with(levels.clone());

        tracing::subscriber::with_default(subscriber, || {
            log_task_error(PROBE_TASK, Ok(()));
        });

        assert_eq!(levels.events().len(), 0);
    }
}
