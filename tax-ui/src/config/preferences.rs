//! The preferences the user can edit: logging, the database connection pool,
//! and the length of the recent connections list.
//!
//! [`PreferencesDraft`] holds what the user typed. [`PreferencesDraft::validate`]
//! converts it into [`Preferences`]. The database URL and backend are not
//! preferences: they change through the Save and Open flows, so
//! [`Preferences::apply_to`] never touches them.

use std::path::PathBuf;
use std::str::FromStr;

use tracing_subscriber::EnvFilter;

use super::{AppConfig, LoggingConfig, PoolSettings};
use crate::logging::SourcePathDisplay;

/// An editable preference. Validation messages name the field they belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceField {
    Level,
    FilePath,
    MaxConnections,
    MinConnections,
    AcquireTimeout,
    IdleTimeout,
    MaxLifetime,
    RecentLimit,
}

/// A problem with one field, shown next to that field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub field: PreferenceField,
    pub message: String,
}

/// Preferences as the user typed them. Numbers stay as text until validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferencesDraft {
    pub level: String,
    pub application_only: bool,
    pub stdout: bool,
    pub file_enabled: bool,
    pub file_path: String,
    pub source_path: SourcePathDisplay,
    pub max_connections: String,
    pub min_connections: String,
    pub acquire_timeout_secs: String,
    pub idle_timeout_secs: String,
    pub max_lifetime_secs: String,
    pub test_before_acquire: bool,
    pub recent_limit: String,
}

/// Preferences after validation. Every value has its final type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preferences {
    pub logging: LoggingConfig,
    pub pool: PoolSettings,
    pub recent_limit: usize,
}

impl PreferencesDraft {
    /// The editable values of `config`, as text.
    pub fn from_config(config: &AppConfig) -> Self {
        let logging = &config.logging;
        let pool = &config.database.pool;

        Self {
            level: logging.level.clone(),
            application_only: logging.application_only,
            stdout: logging.stdout,
            file_enabled: logging.file_enabled,
            file_path: logging.file_path.to_string_lossy().into_owned(),
            source_path: logging.source_path,
            max_connections: pool.max_connections.to_string(),
            min_connections: pool.min_connections.to_string(),
            acquire_timeout_secs: pool.acquire_timeout_secs.to_string(),
            idle_timeout_secs: pool.idle_timeout_secs.to_string(),
            max_lifetime_secs: pool.max_lifetime_secs.to_string(),
            test_before_acquire: pool.test_before_acquire,
            recent_limit: config.recent.limit.to_string(),
        }
    }

    /// Checks every field. Returns the typed preferences, or one error for
    /// each invalid field.
    pub fn validate(&self) -> Result<Preferences, Vec<FieldError>> {
        let mut errors = Vec::new();

        let level = self.level.trim().to_string();
        if level.is_empty() || EnvFilter::try_new(&level).is_err() {
            errors.push(field_error(
                PreferenceField::Level,
                "Enter a level such as info, or a filter such as myapp=debug.",
            ));
        }

        let file_path = self.file_path.trim().to_string();
        if self.file_enabled && file_path.is_empty() {
            errors.push(field_error(
                PreferenceField::FilePath,
                "Enter a file path to log to.",
            ));
        }

        let max_connections = parse_number::<u32>(
            &self.max_connections,
            PreferenceField::MaxConnections,
            &mut errors,
        );
        let min_connections = parse_number::<u32>(
            &self.min_connections,
            PreferenceField::MinConnections,
            &mut errors,
        );
        let acquire_timeout_secs = parse_number::<u64>(
            &self.acquire_timeout_secs,
            PreferenceField::AcquireTimeout,
            &mut errors,
        );
        let idle_timeout_secs = parse_number::<u64>(
            &self.idle_timeout_secs,
            PreferenceField::IdleTimeout,
            &mut errors,
        );
        let max_lifetime_secs = parse_number::<u64>(
            &self.max_lifetime_secs,
            PreferenceField::MaxLifetime,
            &mut errors,
        );
        let recent_limit = parse_number::<usize>(
            &self.recent_limit,
            PreferenceField::RecentLimit,
            &mut errors,
        );

        if max_connections == Some(0) {
            errors.push(field_error(
                PreferenceField::MaxConnections,
                "Enter at least 1.",
            ));
        }

        if let (Some(min), Some(max)) = (min_connections, max_connections) {
            if min > max {
                errors.push(field_error(
                    PreferenceField::MinConnections,
                    "Cannot be more than the maximum connections.",
                ));
            }
        }

        if acquire_timeout_secs == Some(0) {
            errors.push(field_error(
                PreferenceField::AcquireTimeout,
                "Enter at least 1 second.",
            ));
        }

        match (
            max_connections,
            min_connections,
            acquire_timeout_secs,
            idle_timeout_secs,
            max_lifetime_secs,
            recent_limit,
        ) {
            (
                Some(max_connections),
                Some(min_connections),
                Some(acquire_timeout_secs),
                Some(idle_timeout_secs),
                Some(max_lifetime_secs),
                Some(recent_limit),
            ) if errors.is_empty() => Ok(Preferences {
                logging: LoggingConfig {
                    level,
                    application_only: self.application_only,
                    stdout: self.stdout,
                    file_enabled: self.file_enabled,
                    file_path: PathBuf::from(file_path),
                    source_path: self.source_path,
                },
                pool: PoolSettings {
                    max_connections,
                    min_connections,
                    acquire_timeout_secs,
                    idle_timeout_secs,
                    max_lifetime_secs,
                    test_before_acquire: self.test_before_acquire,
                },
                recent_limit,
            }),
            _ => Err(errors),
        }
    }
}

impl Preferences {
    /// Writes these preferences into `config`. The database URL and backend
    /// are left as they are.
    pub fn apply_to(
        self,
        config: &mut AppConfig,
    ) {
        config.logging = self.logging;
        config.database.pool = self.pool;
        config.recent.limit = self.recent_limit;
    }
}

/// Builds a validation error for `field`.
fn field_error(
    field: PreferenceField,
    message: &str,
) -> FieldError {
    FieldError {
        field,
        message: message.to_string(),
    }
}

/// Parses `text` as a number. Adds an error for `field` when it is not one.
fn parse_number<T: FromStr>(
    text: &str,
    field: PreferenceField,
    errors: &mut Vec<FieldError>,
) -> Option<T> {
    match text.trim().parse::<T>() {
        Ok(value) => Some(value),
        Err(_) => {
            errors.push(field_error(field, "Enter a whole number."));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{AppConfig, LoggingConfig, PoolSettings, PreferenceField, PreferencesDraft};

    fn valid_draft() -> PreferencesDraft {
        PreferencesDraft::from_config(&AppConfig::default())
    }

    #[test]
    fn defaults_validate_to_the_default_preferences() {
        let preferences = valid_draft().validate().expect("defaults must be valid");

        assert_eq!(preferences.logging, LoggingConfig::default());
        assert_eq!(preferences.pool, PoolSettings::default());
        assert_eq!(preferences.recent_limit, AppConfig::default().recent.limit);
    }

    #[test]
    fn apply_to_leaves_the_database_url_and_backend_unchanged() {
        let mut config = AppConfig::default();
        config.database.url = "custom.db".to_string();
        let mut draft = PreferencesDraft::from_config(&config);
        draft.level = "debug".to_string();
        let preferences = draft.validate().expect("draft must be valid");

        preferences.apply_to(&mut config);

        assert_eq!(config.database.url, "custom.db");
        assert_eq!(config.logging.level, "debug");
    }

    #[test]
    fn unparsable_number_is_reported_for_its_field() {
        let mut draft = valid_draft();
        draft.max_connections = "many".to_string();

        let errors = draft.validate().expect_err("draft must be invalid");

        assert!(
            errors
                .iter()
                .any(|error| error.field == PreferenceField::MaxConnections)
        );
    }

    #[test]
    fn minimum_above_maximum_is_reported() {
        let mut draft = valid_draft();
        draft.min_connections = "5".to_string();
        draft.max_connections = "2".to_string();

        let errors = draft.validate().expect_err("draft must be invalid");

        assert!(
            errors
                .iter()
                .any(|error| error.field == PreferenceField::MinConnections)
        );
    }

    #[test]
    fn unknown_level_is_reported() {
        let mut draft = valid_draft();
        draft.level = "target=verbose".to_string();

        let errors = draft.validate().expect_err("draft must be invalid");

        assert!(
            errors
                .iter()
                .any(|error| error.field == PreferenceField::Level)
        );
    }

    #[test]
    fn file_path_is_required_only_when_file_logging_is_enabled() {
        let mut draft = valid_draft();
        draft.file_path = String::new();
        assert!(draft.validate().is_ok());

        draft.file_enabled = true;

        let errors = draft.validate().expect_err("draft must be invalid");
        assert!(
            errors
                .iter()
                .any(|error| error.field == PreferenceField::FilePath)
        );
    }
}
