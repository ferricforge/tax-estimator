//! Log level filtering.
//!
//! The startup filter comes from the [`LOG_FILTER_ENV_VAR`] environment
//! variable when it is set and valid, or from a built-in preset that shows the
//! workspace crates at [`DEFAULT_WORKSPACE_LEVEL`] and everything else at
//! [`DEPENDENCY_LEVEL`]. At runtime, [`parse_filter`] accepts either a bare
//! level name or a full `EnvFilter` directive.

use std::env::VarError;
use thiserror::Error;
use tracing::Level;
use tracing_subscriber::{EnvFilter, filter::ParseError};

/// Environment variable that overrides the startup filter with a full
/// `EnvFilter` directive.
pub(super) const LOG_FILTER_ENV_VAR: &str = "RUST_LOG";

/// Event target name of the `tax-core` crate.
const TAX_CORE: &str = "tax_core";

/// Event target name of the `tax-data` crate.
const TAX_DATA: &str = "tax_data";

/// Event target name of the `tax-db-sqlite` crate.
const TAX_DB_SQLITE: &str = "tax_db_sqlite";

/// Event target name of the `tax-ui` crate.
const TAX_UI: &str = "tax_ui";

/// Workspace library crates that emit [`tracing`] events. When adding a new
/// workspace member crate, add a target name constant above and include it
/// here if logs from that crate should follow the default and bare-level
/// presets.
const WORKSPACE_CRATES: &[&str] = &[TAX_CORE, TAX_DATA, TAX_DB_SQLITE, TAX_UI];

/// Level applied to targets outside the workspace crates by the default and
/// bare-level presets.
const DEPENDENCY_LEVEL: Level = Level::WARN;

/// Level applied to the workspace crates when `RUST_LOG` is not set.
const DEFAULT_WORKSPACE_LEVEL: Level = Level::INFO;

/// Levels accepted as bare input by [`parse_filter`].
const BARE_LEVELS: [Level; 5] = [
    Level::ERROR,
    Level::WARN,
    Level::INFO,
    Level::DEBUG,
    Level::TRACE,
];

/// Separates directives within a filter string.
const DIRECTIVE_SEPARATOR: char = ',';

/// Separates a target from its level within a directive.
const TARGET_LEVEL_SEPARATOR: char = '=';

/// Error returned when filter input cannot be turned into an [`EnvFilter`].
#[derive(Debug, Error)]
pub(super) enum FilterError {
    /// The input was empty or contained only whitespace.
    #[error("empty log filter")]
    Empty,

    /// The input was not a bare level and could not be parsed as a directive.
    #[error("invalid log filter '{input}'")]
    InvalidDirective {
        input: String,
        #[source]
        source: ParseError,
    },

    /// A directive built from a bare level failed to parse. This indicates a
    /// defect in the directive builder rather than invalid input.
    #[error("invalid built-in workspace filter '{directive}'")]
    InvalidPreset {
        directive: String,
        #[source]
        source: ParseError,
    },

    /// The input was not valid Unicode.
    #[error("log filter is not valid Unicode")]
    NotUnicode,
}

/// Where the startup filter came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LogFilterSource {
    /// The built-in preset.
    Default,

    /// A valid directive read from [`LOG_FILTER_ENV_VAR`].
    Environment,
}

/// Target scope used when a filter is specified as a bare level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LevelScope {
    /// Apply the level to the application workspace crates and keep all other
    /// targets at [`DEPENDENCY_LEVEL`].
    ApplicationOnly,

    /// Apply the level to every target.
    Global,
}

/// Filter to install at startup, where it came from, and the reason the
/// environment value was ignored, if it was.
pub(super) struct InitialFilter {
    /// Filter to install.
    pub(super) filter: EnvFilter,

    /// Where `filter` came from.
    pub(super) source: LogFilterSource,

    /// Why [`LOG_FILTER_ENV_VAR`] was ignored, when it was set but unusable.
    pub(super) rejected_env: Option<FilterError>,
}

impl InitialFilter {
    /// The built-in preset, with the reason the environment value was
    /// ignored, if it was.
    fn default_preset(rejected_env: Option<FilterError>) -> Self {
        Self {
            filter: default_filter(),
            source: LogFilterSource::Default,
            rejected_env,
        }
    }
}

/// Returns the filter to install at startup.
///
/// Uses [`LOG_FILTER_ENV_VAR`] as a full directive when it is set. When it is
/// not set, or cannot be used, applies [`DEFAULT_WORKSPACE_LEVEL`] to the
/// workspace crates.
pub(super) fn initial_filter() -> InitialFilter {
    initial_filter_from(std::env::var(LOG_FILTER_ENV_VAR))
}

/// Builds the startup filter from the value read for [`LOG_FILTER_ENV_VAR`].
fn initial_filter_from(env_value: Result<String, VarError>) -> InitialFilter {
    let env_filter = match env_value {
        Ok(directive) => directive_filter(&directive).map(Some),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(FilterError::NotUnicode),
    };

    match env_filter {
        Ok(Some(filter)) => InitialFilter {
            filter,
            source: LogFilterSource::Environment,
            rejected_env: None,
        },
        Ok(None) => InitialFilter::default_preset(None),
        Err(error) => InitialFilter::default_preset(Some(error)),
    }
}

/// Returns the built-in startup filter.
fn default_filter() -> EnvFilter {
    application_level_filter(DEFAULT_WORKSPACE_LEVEL).expect("built-in default filter must parse")
}

/// Returns the bare level name of [`DEFAULT_WORKSPACE_LEVEL`].
pub(super) fn default_level_name() -> &'static str {
    directive_name(DEFAULT_WORKSPACE_LEVEL)
}

/// Parses runtime filter input.
///
/// A bare level (one of [`BARE_LEVELS`], in any letter case, with optional
/// surrounding whitespace) uses `level_scope` to decide whether it applies to
/// the application workspace crates or every target. Any other input is parsed
/// as a full `EnvFilter` directive and defines its own target scopes.
pub(super) fn parse_filter(
    input: &str,
    level_scope: LevelScope,
) -> Result<EnvFilter, FilterError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(FilterError::Empty);
    }

    match (parse_bare_level(trimmed), level_scope) {
        (Some(level), LevelScope::ApplicationOnly) => application_level_filter(level),
        (Some(level), LevelScope::Global) => global_level_filter(level),
        (None, _) => directive_filter(input),
    }
}

/// Returns the level named by `input` if it is one of [`BARE_LEVELS`],
/// ignoring letter case.
fn parse_bare_level(input: &str) -> Option<Level> {
    BARE_LEVELS
        .into_iter()
        .find(|level| input.eq_ignore_ascii_case(directive_name(*level)))
}

/// Builds an application-only filter for a bare level.
fn application_level_filter(level: Level) -> Result<EnvFilter, FilterError> {
    let directive = bare_level_directive(level);
    EnvFilter::try_new(&directive)
        .map_err(|source| FilterError::InvalidPreset { directive, source })
}

/// Builds a global filter for a bare level.
fn global_level_filter(level: Level) -> Result<EnvFilter, FilterError> {
    let directive = directive_name(level).to_string();
    EnvFilter::try_new(&directive)
        .map_err(|source| FilterError::InvalidPreset { directive, source })
}

/// Builds the filter for a full `EnvFilter` directive.
fn directive_filter(input: &str) -> Result<EnvFilter, FilterError> {
    EnvFilter::try_new(input).map_err(|source| FilterError::InvalidDirective {
        input: input.to_string(),
        source,
    })
}

/// Builds a directive that applies `level` to every workspace crate and
/// [`DEPENDENCY_LEVEL`] to all other targets.
fn bare_level_directive(level: Level) -> String {
    let level_name = directive_name(level);
    let mut directive = String::from(directive_name(DEPENDENCY_LEVEL));
    for crate_name in WORKSPACE_CRATES {
        directive.push(DIRECTIVE_SEPARATOR);
        directive.push_str(crate_name);
        directive.push(TARGET_LEVEL_SEPARATOR);
        directive.push_str(level_name);
    }
    directive
}

/// Returns the name that `EnvFilter` directives use for `level`.
fn directive_name(level: Level) -> &'static str {
    match level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::ffi::OsString;
    use tracing_subscriber::layer::SubscriberExt;

    use crate::logging::test_support::{PROBE_MESSAGE, RecordingLayer, recorded};

    /// Event target of a dependency crate.
    const SQLX_TARGET: &str = "sqlx";

    /// Event target of a second dependency crate.
    const GPUI_TARGET: &str = "gpui";

    /// A directive with a level name that `EnvFilter` does not accept.
    const INVALID_DIRECTIVE: &str = "tax_ui=loud";

    // --- Test helpers ---

    /// Runs `emit` with a scoped subscriber that applies `filter`, and returns
    /// the target and level of each event that passed the filter.
    fn record_filtered_events<F>(
        filter: EnvFilter,
        emit: F,
    ) -> Vec<(String, Level)>
    where
        F: FnOnce(),
    {
        let recorder = RecordingLayer::default();
        let subscriber = tracing_subscriber::registry()
            .with(filter)
            .with(recorder.clone());

        tracing::subscriber::with_default(subscriber, emit);

        recorder.events()
    }

    // --- Directive building ---

    #[test]
    fn directive_name_maps_each_bare_level() {
        let actual: Vec<&str> = BARE_LEVELS.into_iter().map(directive_name).collect();
        assert_eq!(actual, vec!["error", "warn", "info", "debug", "trace"]);
    }

    #[test]
    fn default_level_name_is_info() {
        assert_eq!(default_level_name(), "info");
    }

    #[test]
    fn bare_level_directive_matches_expected_shape() {
        assert_eq!(
            bare_level_directive(Level::INFO),
            "warn,tax_core=info,tax_data=info,tax_db_sqlite=info,tax_ui=info"
        );
    }

    #[test]
    fn default_preset_passes_workspace_info_and_dependency_warn() {
        let filter = application_level_filter(DEFAULT_WORKSPACE_LEVEL).expect("preset must parse");
        let actual = record_filtered_events(filter, || {
            tracing::info!(target: TAX_CORE, "{PROBE_MESSAGE}");
            tracing::debug!(target: TAX_UI, "{PROBE_MESSAGE}");
            tracing::warn!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
            tracing::info!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
        });
        let expected = vec![
            recorded(TAX_CORE, Level::INFO),
            recorded(SQLX_TARGET, Level::WARN),
        ];

        assert_eq!(actual, expected);
    }

    // --- Startup filter ---

    #[test]
    fn initial_filter_from_unset_variable_uses_the_default_preset() {
        let initial = initial_filter_from(Err(VarError::NotPresent));

        assert_eq!(initial.source, LogFilterSource::Default);
        if let Some(error) = initial.rejected_env {
            panic!("expected no rejection, got {error:?}");
        }

        let actual = record_filtered_events(initial.filter, || {
            tracing::info!(target: TAX_UI, "{PROBE_MESSAGE}");
            tracing::info!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
        });

        assert_eq!(actual, vec![recorded(TAX_UI, Level::INFO)]);
    }

    #[test]
    fn initial_filter_from_valid_variable_uses_it_as_a_full_directive() {
        let initial = initial_filter_from(Ok(format!("{SQLX_TARGET}=debug")));

        assert_eq!(initial.source, LogFilterSource::Environment);
        if let Some(error) = initial.rejected_env {
            panic!("expected no rejection, got {error:?}");
        }

        let actual = record_filtered_events(initial.filter, || {
            tracing::debug!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
            // The directive names only `sqlx`, so workspace crates are filtered out.
            tracing::info!(target: TAX_UI, "{PROBE_MESSAGE}");
        });

        assert_eq!(actual, vec![recorded(SQLX_TARGET, Level::DEBUG)]);
    }

    #[test]
    fn initial_filter_from_invalid_variable_falls_back_to_the_default_preset() {
        let initial = initial_filter_from(Ok(INVALID_DIRECTIVE.to_string()));

        assert_eq!(initial.source, LogFilterSource::Default);
        match initial.rejected_env {
            Some(FilterError::InvalidDirective { input, .. }) => {
                assert_eq!(input, INVALID_DIRECTIVE);
            }
            other => panic!("expected FilterError::InvalidDirective, got {other:?}"),
        }

        let actual = record_filtered_events(initial.filter, || {
            tracing::info!(target: TAX_UI, "{PROBE_MESSAGE}");
        });

        assert_eq!(actual, vec![recorded(TAX_UI, Level::INFO)]);
    }

    #[test]
    fn initial_filter_from_non_unicode_variable_falls_back_to_the_default_preset() {
        let initial = initial_filter_from(Err(VarError::NotUnicode(OsString::new())));

        assert_eq!(initial.source, LogFilterSource::Default);
        match initial.rejected_env {
            Some(FilterError::NotUnicode) => {}
            other => panic!("expected FilterError::NotUnicode, got {other:?}"),
        }

        let actual = record_filtered_events(initial.filter, || {
            tracing::info!(target: TAX_UI, "{PROBE_MESSAGE}");
        });

        assert_eq!(actual, vec![recorded(TAX_UI, Level::INFO)]);
    }

    // --- Parsing ---

    #[test]
    fn parse_filter_bare_level_applies_only_to_workspace_crates() {
        let filter =
            parse_filter("debug", LevelScope::ApplicationOnly).expect("bare level must parse");
        let actual = record_filtered_events(filter, || {
            tracing::debug!(target: TAX_DB_SQLITE, "{PROBE_MESSAGE}");
            tracing::trace!(target: TAX_DB_SQLITE, "{PROBE_MESSAGE}");
            tracing::warn!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
            tracing::info!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
        });
        let expected = vec![
            recorded(TAX_DB_SQLITE, Level::DEBUG),
            recorded(GPUI_TARGET, Level::WARN),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_filter_bare_level_applies_globally_when_requested() {
        let filter = parse_filter("debug", LevelScope::Global).expect("bare level must parse");
        let actual = record_filtered_events(filter, || {
            tracing::debug!(target: TAX_UI, "{PROBE_MESSAGE}");
            tracing::debug!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
            tracing::trace!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
        });
        let expected = vec![
            recorded(TAX_UI, Level::DEBUG),
            recorded(GPUI_TARGET, Level::DEBUG),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_filter_bare_level_ignores_case_and_surrounding_whitespace() {
        // If this input were parsed as a full directive, `DEBUG` would apply to
        // every target and the `sqlx` event would pass.
        let filter =
            parse_filter("  DEBUG ", LevelScope::ApplicationOnly).expect("bare level must parse");
        let actual = record_filtered_events(filter, || {
            tracing::debug!(target: TAX_UI, "{PROBE_MESSAGE}");
            tracing::info!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
        });

        assert_eq!(actual, vec![recorded(TAX_UI, Level::DEBUG)]);
    }

    #[test]
    fn parse_filter_full_directive_is_used_as_written() {
        let input = format!("{GPUI_TARGET}=warn,{TAX_UI}=debug");
        let filter =
            parse_filter(&input, LevelScope::ApplicationOnly).expect("full directive must parse");
        let actual = record_filtered_events(filter, || {
            tracing::debug!(target: TAX_UI, "{PROBE_MESSAGE}");
            tracing::info!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
            tracing::warn!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
            // No directive matches `sqlx`, so even an error is filtered out.
            tracing::error!(target: SQLX_TARGET, "{PROBE_MESSAGE}");
        });
        let expected = vec![
            recorded(TAX_UI, Level::DEBUG),
            recorded(GPUI_TARGET, Level::WARN),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_filter_full_directive_ignores_bare_level_scope() {
        let input = format!("{GPUI_TARGET}=debug");
        let application_only =
            parse_filter(&input, LevelScope::ApplicationOnly).expect("full directive must parse");
        let global = parse_filter(&input, LevelScope::Global).expect("full directive must parse");

        for filter in [application_only, global] {
            let actual = record_filtered_events(filter, || {
                tracing::debug!(target: GPUI_TARGET, "{PROBE_MESSAGE}");
                tracing::error!(target: TAX_UI, "{PROBE_MESSAGE}");
            });
            assert_eq!(actual, vec![recorded(GPUI_TARGET, Level::DEBUG)]);
        }
    }

    #[test]
    fn parse_filter_rejects_empty_input() {
        match parse_filter("   ", LevelScope::ApplicationOnly).unwrap_err() {
            FilterError::Empty => {}
            other => panic!("expected FilterError::Empty, got {other:?}"),
        }
    }

    #[test]
    fn parse_filter_rejects_invalid_directive() {
        match parse_filter(INVALID_DIRECTIVE, LevelScope::ApplicationOnly).unwrap_err() {
            FilterError::InvalidDirective { input, .. } => assert_eq!(input, INVALID_DIRECTIVE),
            other => panic!("expected FilterError::InvalidDirective, got {other:?}"),
        }
    }
}
