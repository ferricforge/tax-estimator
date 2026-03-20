use anyhow::Result;
use chrono::Local;
use std::{
    fs::File,
    io::{self, IsTerminal, Write},
    path::Path,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};
use tracing::{Event, Level, Subscriber, error};
use tracing_subscriber::{
    EnvFilter,
    Layer, // Layer is used by .with_filter() on the stdout layer below
    fmt::{
        FmtContext, MakeWriter,
        format::{FormatEvent, FormatFields, Writer},
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    reload,
    util::SubscriberInitExt,
};

// --- Formatter (unchanged) ---

struct LocalFmt;

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
    ) -> std::fmt::Result {
        let meta = event.metadata();
        let ansi = writer.has_ansi_escapes();

        if ansi {
            write!(writer, "\x1b[2m")?
        }
        write!(
            writer,
            "{} ",
            Local::now().format("%Y-%m-%dT%H:%M:%S%.6f%:z")
        )?;
        if ansi {
            write!(writer, "\x1b[0m")?
        }

        let (pre, post) = if ansi {
            match *meta.level() {
                Level::ERROR => ("\x1b[1;31m", "\x1b[0m"),
                Level::WARN => ("\x1b[1;33m", "\x1b[0m"),
                Level::INFO => ("\x1b[1;32m", "\x1b[0m"),
                Level::DEBUG => ("\x1b[1;34m", "\x1b[0m"),
                Level::TRACE => ("\x1b[1;35m", "\x1b[0m"),
            }
        } else {
            ("", "")
        };
        write!(writer, "{}{:>5}{} ", pre, meta.level(), post)?;

        let file = meta.file().map(|f| {
            f.strip_prefix("src/")
                .or_else(|| f.strip_prefix("src\\"))
                .unwrap_or(f)
        });
        if let (Some(file), Some(line)) = (file, meta.line()) {
            if ansi {
                write!(writer, "\x1b[36m{file}:{line}\x1b[0m ")?;
            } else {
                write!(writer, "{file}:{line} ")?;
            }
        }

        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

// --- Late-bound file writer ---

/// A MakeWriter that can be pointed at a file after initialization.
/// While no file is set, all writes are silently discarded.
#[derive(Clone)]
struct FileSlot(Arc<Mutex<Option<File>>>);

struct SlotWriter<'a>(MutexGuard<'a, Option<File>>);

impl Write for SlotWriter<'_> {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> io::Result<usize> {
        match &mut *self.0 {
            Some(f) => f.write(buf),
            None => Ok(buf.len()), // discard silently when no file is set
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match &mut *self.0 {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for FileSlot {
    type Writer = SlotWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        SlotWriter(self.0.lock().unwrap())
    }
}

// --- Statics ---

type SetStrFn = Box<dyn Fn(&str) -> Result<()> + Send + Sync>;
type SetBoolFn = Box<dyn Fn(bool) -> Result<()> + Send + Sync>;

static APP_NAME: OnceLock<String> = OnceLock::new();
static SET_LOG_LEVEL: OnceLock<SetStrFn> = OnceLock::new();
static SET_STDOUT_ENABLED: OnceLock<SetBoolFn> = OnceLock::new();
static FILE_SLOT: OnceLock<Arc<Mutex<Option<File>>>> = OnceLock::new();

fn make_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,gpui_demo=debug"))
}

fn store_level_handle<S>(handle: reload::Handle<EnvFilter, S>)
where
    S: Subscriber + Send + Sync + 'static,
{
    let _ = SET_LOG_LEVEL.set(Box::new(move |level_str: &str| {
        let filter = EnvFilter::try_new(level_str)
            .map_err(|e| anyhow::anyhow!("invalid log level '{level_str}': {e}"))?;
        handle
            .reload(filter)
            .map_err(|e| anyhow::anyhow!("filter reload failed: {e}"))
    }));
}

fn store_stdout_handle<S>(handle: reload::Handle<EnvFilter, S>)
where
    S: Subscriber + Send + Sync + 'static,
{
    let _ = SET_STDOUT_ENABLED.set(Box::new(move |enabled: bool| {
        // "trace" passes everything through; the global filter is still the ceiling.
        let filter = if enabled {
            EnvFilter::new("trace")
        } else {
            EnvFilter::new("off")
        };
        handle
            .reload(filter)
            .map_err(|e| anyhow::anyhow!("stdout reload failed: {e}"))
    }));
}

// --- Public API ---

/// Changes the active log filter at runtime.
/// Accepts a bare level ("error", "warn", "info", "debug", "trace")
/// or any full EnvFilter directive. Case-insensitive.
pub fn set_log_level(level: &str) -> Result<()> {
    match SET_LOG_LEVEL.get() {
        Some(f) => f(level),
        None => anyhow::bail!("logging not yet initialized"),
    }
}

/// Shows or hides stdout log output without affecting file logging.
pub fn set_stdout_enabled(enabled: bool) -> Result<()> {
    match SET_STDOUT_ENABLED.get() {
        Some(f) => f(enabled),
        None => anyhow::bail!("logging not yet initialized"),
    }
}

/// Starts writing log output to `path`. Safe to call after initialization.
/// If a file is already open it is replaced.
/// The directory must already exist.
pub fn enable_file_logging(path: &Path) -> Result<()> {
    let file = File::options()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| anyhow::anyhow!("cannot open log file '{}': {e}", path.display()))?;

    match FILE_SLOT.get() {
        Some(slot) => {
            *slot.lock().unwrap() = Some(file);
            Ok(())
        }
        None => anyhow::bail!("logging not yet initialized"),
    }
}

/// Closes the current log file. Subsequent records go to stdout only
/// (if stdout is enabled).
pub fn disable_file_logging() {
    if let Some(slot) = FILE_SLOT.get() {
        *slot.lock().unwrap() = None;
    }
}

/// Returns the process name derived from the executable path.
/// Initialised on first call; always returns the same value.
/// Falls back to "app" if the path cannot be determined.
/// Exposed so a future preferences store can read it without re-deriving it.
pub fn app_name() -> &'static str {
    APP_NAME.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "app".to_string())
    })
}

/// Initializes logging. Call once at startup.
///
/// - Stdout: colored when attached to a terminal, plain when piped.
/// - File: inactive until `enable_file_logging()` is called.
/// - Level: INFO by default, or overridden by the RUST_LOG env var.
pub fn init_default_logging() {
    // Initialise the name while we are still in a simple synchronous context.
    let _ = app_name();

    let file_inner: Arc<Mutex<Option<File>>> = Arc::new(Mutex::new(None));
    let _ = FILE_SLOT.set(file_inner.clone());

    // Per-stdout on/off filter; starts open ("trace"). Global filter is still the ceiling.
    let (stdout_gate, stdout_handle) = reload::Layer::new(EnvFilter::new("trace"));
    // Global level filter; controls both layers simultaneously.
    let (level_filter, level_handle) = reload::Layer::new(make_filter());

    let stdout_layer = tracing_subscriber::fmt::layer()
        .event_format(LocalFmt)
        .with_ansi(io::stdout().is_terminal())
        .with_filter(stdout_gate); // Layer trait used here — import is not dead

    let file_layer = tracing_subscriber::fmt::layer()
        .event_format(LocalFmt)
        .with_ansi(false)
        .with_writer(FileSlot(file_inner)); // discards until enable_file_logging() is called

    if tracing_subscriber::registry()
        .with(level_filter) // global level ceiling
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .is_ok()
    {
        store_level_handle(level_handle);
        store_stdout_handle(stdout_handle);
    }
}

/// Logs a background task failure with context.
pub fn log_task_error(
    task_name: &'static str,
    result: Result<()>,
) {
    if let Err(error) = result {
        error!(task = task_name, ?error, "background task failed");
    }
}
