//! Event formatting for log output.
//!
//! Each event is written as one line:
//! `<timestamp> <level> <file>:<line> <message and fields>`.
//! When the writer supports ANSI escapes, the timestamp is dimmed, the level
//! is colored by severity, and the source location is cyan. How the source
//! file is shown is controlled by [`SourcePathDisplay`].

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::{
    fmt::{
        FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    registry::LookupSpan,
};

/// Local time with microseconds and a UTC offset, for example
/// `2026-09-16T19:38:58.674344-05:00`.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6f%:z";

/// Minimum width of the level column. Shorter level names are right-aligned
/// so that the columns after the level line up.
const LEVEL_WIDTH: usize = 5;

/// Written after each leading segment: timestamp, level, and source location.
const SEGMENT_SEPARATOR: &str = " ";

/// Name of the source directory that [`SourcePathDisplay::Short`] removes,
/// together with everything before it.
const SOURCE_DIR_NAME: &str = "src";

/// Characters that separate segments in source paths.
const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

/// ANSI escape sequences used when the writer supports them.
mod ansi {
    /// Clears all styles.
    pub(super) const RESET: &str = "\x1b[0m";

    /// Dim text.
    pub(super) const DIM: &str = "\x1b[2m";

    /// Cyan text.
    pub(super) const CYAN: &str = "\x1b[36m";

    /// Bold red text.
    pub(super) const BOLD_RED: &str = "\x1b[1;31m";

    /// Bold yellow text.
    pub(super) const BOLD_YELLOW: &str = "\x1b[1;33m";

    /// Bold green text.
    pub(super) const BOLD_GREEN: &str = "\x1b[1;32m";

    /// Bold blue text.
    pub(super) const BOLD_BLUE: &str = "\x1b[1;34m";

    /// Bold magenta text.
    pub(super) const BOLD_MAGENTA: &str = "\x1b[1;35m";
}

/// How the source location of an event is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePathDisplay {
    /// The path as recorded at compile time, for example
    /// `tax-ui/src/components/window/mod.rs`.
    #[default]
    Full,

    /// The part after the last `src` directory, for example
    /// `components/window/mod.rs`.
    Short,

    /// Only the file name, for example `mod.rs`.
    FileName,

    /// No source location.
    Hidden,
}

impl SourcePathDisplay {
    /// Every mode, in declaration order. The position of each mode is the
    /// value stored for it in [`SourcePathHandle`].
    const ALL: [Self; 4] = [Self::Full, Self::Short, Self::FileName, Self::Hidden];

    /// Returns how `file` is shown, or `None` when the location is hidden.
    fn display_path(
        self,
        file: &str,
    ) -> Option<&str> {
        match self {
            Self::Full => Some(file),
            Self::Short => Some(after_src_segment(file)),
            Self::FileName => Some(file_name(file)),
            Self::Hidden => None,
        }
    }
}

/// Source path display mode shared by the formatters of every output, so a
/// change applies to all of them at once.
#[derive(Clone, Debug, Default)]
pub(super) struct SourcePathHandle(Arc<AtomicU8>);

impl SourcePathHandle {
    /// Returns the current mode.
    pub(super) fn get(&self) -> SourcePathDisplay {
        let value = usize::from(self.0.load(Ordering::Relaxed));
        SourcePathDisplay::ALL
            .get(value)
            .copied()
            .unwrap_or_default()
    }

    /// Replaces the current mode.
    pub(super) fn set(
        &self,
        display: SourcePathDisplay,
    ) {
        self.0.store(display as u8, Ordering::Relaxed);
    }
}

/// Formats events with a local timestamp, severity level, source location,
/// and event fields.
pub(super) struct LocalFmt {
    source_path: SourcePathHandle,
}

impl LocalFmt {
    /// Builds a formatter that shows source locations as `source_path`
    /// currently specifies.
    pub(super) fn new(source_path: SourcePathHandle) -> Self {
        Self { source_path }
    }
}

impl<S, N> FormatEvent<S, N> for LocalFmt
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let meta = event.metadata();
        let level = *meta.level();

        let timestamp = Local::now().format(TIMESTAMP_FORMAT);
        write_segment(&mut writer, ansi::DIM, timestamp)?;

        let style = level_style(level);
        write_segment(&mut writer, style, format_args!("{level:>LEVEL_WIDTH$}"))?;

        if let Some((path, line)) = source_location(meta, self.source_path.get()) {
            write_segment(&mut writer, ansi::CYAN, format_args!("{path}:{line}"))?;
        }

        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Writes `text` followed by [`SEGMENT_SEPARATOR`]. When the writer supports
/// ANSI escapes, `text` is wrapped in `style` and [`ansi::RESET`].
fn write_segment(
    writer: &mut Writer<'_>,
    style: &str,
    text: impl Display,
) -> fmt::Result {
    if writer.has_ansi_escapes() {
        write!(writer, "{style}{text}{}{SEGMENT_SEPARATOR}", ansi::RESET)
    } else {
        write!(writer, "{text}{SEGMENT_SEPARATOR}")
    }
}

/// Returns the ANSI style for a severity level.
fn level_style(level: Level) -> &'static str {
    match level {
        Level::ERROR => ansi::BOLD_RED,
        Level::WARN => ansi::BOLD_YELLOW,
        Level::INFO => ansi::BOLD_GREEN,
        Level::DEBUG => ansi::BOLD_BLUE,
        Level::TRACE => ansi::BOLD_MAGENTA,
    }
}

/// Returns the source path and line of an event as `display` shows them, or
/// `None` when the event has no location or the location is hidden.
fn source_location<'a>(
    meta: &Metadata<'a>,
    display: SourcePathDisplay,
) -> Option<(&'a str, u32)> {
    let path = display.display_path(meta.file()?)?;
    Some((path, meta.line()?))
}

/// Returns the part of `file` after its last `src` directory segment, or
/// `file` unchanged when it has none.
fn after_src_segment(file: &str) -> &str {
    let mut start = None;
    for (index, _) in file.match_indices(SOURCE_DIR_NAME) {
        let end = index + SOURCE_DIR_NAME.len();
        let begins_segment = index == 0 || file[..index].ends_with(PATH_SEPARATORS);
        if begins_segment && file[end..].starts_with(PATH_SEPARATORS) {
            start = Some(end + 1);
        }
    }
    start.map_or(file, |start| &file[start..])
}

/// Returns the last segment of `file`.
fn file_name(file: &str) -> &str {
    file.rfind(PATH_SEPARATORS)
        .map_or(file, |index| &file[index + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::DateTime;
    use pretty_assertions::assert_eq;
    use regex::Regex;
    use std::{
        io::{self, Write},
        sync::{Mutex, MutexGuard},
    };
    use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt};

    // Expected ANSI sequences, written out independently of the `ansi` module
    // so that an accidental change to those constants fails the tests.
    const ESC_RESET: &str = "\x1b[0m";
    const ESC_DIM: &str = "\x1b[2m";
    const ESC_CYAN: &str = "\x1b[36m";

    /// Expected style for each level, in order from least to most verbose.
    const LEVEL_STYLES: [(Level, &str); 5] = [
        (Level::ERROR, "\x1b[1;31m"),
        (Level::WARN, "\x1b[1;33m"),
        (Level::INFO, "\x1b[1;32m"),
        (Level::DEBUG, "\x1b[1;34m"),
        (Level::TRACE, "\x1b[1;35m"),
    ];

    /// Message used by every probe event.
    const PROBE_MESSAGE: &str = "formatter probe";

    /// Field value used by the plain output probe event.
    const PROBE_ANSWER: u32 = 42;

    // --- Test helpers ---

    /// In-memory `MakeWriter` that collects formatted log output.
    #[derive(Clone, Default)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    struct CaptureGuard<'a>(MutexGuard<'a, Vec<u8>>);

    impl Write for CaptureGuard<'_> {
        fn write(
            &mut self,
            buf: &[u8],
        ) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureGuard<'a>;

        fn make_writer(&'a self) -> Self::Writer {
            CaptureGuard(self.0.lock().unwrap())
        }
    }

    impl CaptureWriter {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).expect("log output must be UTF-8")
        }
    }

    /// Runs `emit` with a scoped subscriber that formats events with
    /// `LocalFmt` using `display`, and returns everything that was written.
    fn capture_formatted<F>(
        ansi: bool,
        display: SourcePathDisplay,
        emit: F,
    ) -> String
    where
        F: FnOnce(),
    {
        let handle = SourcePathHandle::default();
        handle.set(display);

        let capture = CaptureWriter::default();
        let layer = tracing_subscriber::fmt::layer()
            .event_format(LocalFmt::new(handle))
            .with_ansi(ansi)
            .with_writer(capture.clone());
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, emit);

        capture.contents()
    }

    /// Finds the timestamp in `line` and confirms that it matches
    /// [`TIMESTAMP_FORMAT`] exactly by parsing and formatting it again.
    fn extract_timestamp(line: &str) -> &str {
        let pattern =
            Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+[+-]\d{2}:\d{2}").unwrap();
        let timestamp = pattern
            .find(line)
            .expect("line must contain a timestamp")
            .as_str();
        let parsed = DateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT)
            .expect("timestamp must parse with TIMESTAMP_FORMAT");

        assert_eq!(parsed.format(TIMESTAMP_FORMAT).to_string(), timestamp);

        timestamp
    }

    // --- Helper functions ---

    #[test]
    fn level_style_maps_each_level_to_its_color() {
        let actual: Vec<(Level, &str)> = LEVEL_STYLES
            .into_iter()
            .map(|(level, _)| (level, level_style(level)))
            .collect();

        assert_eq!(actual, LEVEL_STYLES);
    }

    #[test]
    fn display_path_applies_each_mode() {
        let file = "tax-ui/src/window/mod.rs";
        let actual: Vec<Option<&str>> = SourcePathDisplay::ALL
            .into_iter()
            .map(|display| display.display_path(file))
            .collect();
        let expected = vec![Some(file), Some("window/mod.rs"), Some("mod.rs"), None];

        assert_eq!(actual, expected);
    }

    #[test]
    fn after_src_segment_removes_through_the_last_src_segment() {
        let cases = [
            ("src/lib.rs", "lib.rs"),
            ("tax-ui/src/window/mod.rs", "window/mod.rs"),
            ("tax-ui\\src\\lib.rs", "lib.rs"),
            ("/registry/src/gpui-0.2.2/src/window.rs", "window.rs"),
            ("srcs/lib.rs", "srcs/lib.rs"),
            ("lib.rs", "lib.rs"),
        ];
        let actual: Vec<(&str, &str)> = cases
            .into_iter()
            .map(|(input, _)| (input, after_src_segment(input)))
            .collect();

        assert_eq!(actual, cases);
    }

    #[test]
    fn file_name_returns_the_last_segment() {
        let cases = [
            ("tax-ui/src/lib.rs", "lib.rs"),
            ("tax-ui\\src\\lib.rs", "lib.rs"),
            ("lib.rs", "lib.rs"),
        ];
        let actual: Vec<(&str, &str)> = cases
            .into_iter()
            .map(|(input, _)| (input, file_name(input)))
            .collect();

        assert_eq!(actual, cases);
    }

    #[test]
    fn source_path_handle_defaults_to_full_and_stores_each_mode() {
        let handle = SourcePathHandle::default();
        let mut actual = vec![handle.get()];
        for display in SourcePathDisplay::ALL {
            handle.set(display);
            actual.push(handle.get());
        }

        let mut expected = vec![SourcePathDisplay::Full];
        expected.extend(SourcePathDisplay::ALL);

        assert_eq!(SourcePathDisplay::default(), SourcePathDisplay::Full);
        assert_eq!(actual, expected);
    }

    // --- Formatter ---

    #[test]
    fn local_fmt_plain_output_has_timestamp_level_location_and_message() {
        let mut probe_line = 0;
        let output = capture_formatted(false, SourcePathDisplay::Full, || {
            // The event must be on the line directly after this assignment.
            probe_line = line!() + 1;
            tracing::info!(answer = PROBE_ANSWER, "{PROBE_MESSAGE}");
        });

        // This literal layout is the specification for plain output. With
        // `SourcePathDisplay::Full`, the location matches `file!()`.
        let timestamp = extract_timestamp(&output);
        let expected = format!(
            "{timestamp}  INFO {}:{probe_line} {PROBE_MESSAGE} answer={PROBE_ANSWER}\n",
            file!()
        );

        assert_eq!(output, expected);
    }

    #[test]
    fn local_fmt_ansi_output_styles_timestamp_level_and_location() {
        let mut first_line = 0;
        let output = capture_formatted(true, SourcePathDisplay::Full, || {
            // The events must start on the line directly after this assignment.
            first_line = line!() + 1;
            tracing::error!("{PROBE_MESSAGE}");
            tracing::warn!("{PROBE_MESSAGE}");
            tracing::info!("{PROBE_MESSAGE}");
            tracing::debug!("{PROBE_MESSAGE}");
            tracing::trace!("{PROBE_MESSAGE}");
        });
        let actual: Vec<&str> = output.lines().collect();
        let timestamps: Vec<&str> = output.lines().map(extract_timestamp).collect();
        assert_eq!(actual.len(), LEVEL_STYLES.len());

        let file = file!();
        let mut expected = Vec::new();
        for (index, (level, style)) in LEVEL_STYLES.into_iter().enumerate() {
            let timestamp = timestamps[index];
            let line = first_line + index as u32;
            let styled_level = format!("{style}{level:>LEVEL_WIDTH$}{ESC_RESET}");
            let location = format!("{ESC_CYAN}{file}:{line}{ESC_RESET}");
            expected.push(format!(
                "{ESC_DIM}{timestamp}{ESC_RESET} {styled_level} {location} {PROBE_MESSAGE}"
            ));
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn local_fmt_hidden_source_path_omits_the_location() {
        let output = capture_formatted(false, SourcePathDisplay::Hidden, || {
            tracing::info!("{PROBE_MESSAGE}");
        });

        let timestamp = extract_timestamp(&output);
        assert_eq!(output, format!("{timestamp}  INFO {PROBE_MESSAGE}\n"));
    }
}
