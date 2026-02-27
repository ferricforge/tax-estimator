use std::{fmt, path::PathBuf};

/// Represents the collected values from the file selection form.
#[derive(Clone, Debug, Default)]
pub struct FileFormModel {
    pub source_file: PathBuf,
    pub database_file: PathBuf,
    pub log_directory: PathBuf,
    pub log_stdout: bool,
}

impl FileFormModel {
    /// Returns `true` if the source file has an Excel extension.
    pub fn is_excel(&self) -> bool {
        matches!(
            self.source_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref(),
            Some("xlsx" | "xlsm" | "xlsb" | "xls")
        )
    }

    /// Returns `true` if the source file has an CSV extension.
    pub fn is_csv(&self) -> bool {
        matches!(
            self.source_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref(),
            Some("csv")
        )
    }

    /// Returns `true` if the database file has a SQLite extension.
    pub fn is_sqlite(&self) -> bool {
        matches!(
            self.database_file
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref(),
            Some("db" | "db3" | "sqlite")
        )
    }

    /// Validates that the model has all required values for submission.
    ///
    /// Rules:
    /// - source file is required
    /// - database file is required
    /// - selected sheet is required only for Excel source files
    pub fn validate_for_submit(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.source_file.as_os_str().is_empty() {
            errors.push("Source file is required.".to_string());
        }

        if self.database_file.as_os_str().is_empty() {
            errors.push("Database file is required.".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl fmt::Display for FileFormModel {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        writeln!(f, "Source file:   {}", self.source_file.to_string_lossy())?;
        writeln!(f, "Database:      {}", self.database_file.to_string_lossy())?;
        writeln!(f, "Log folder:    {}", self.log_directory.to_string_lossy())?;
        writeln!(f, "Log to stdout: {}", self.log_stdout)
    }
}
