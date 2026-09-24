//! Connection-file operations that do not depend on the UI.
//!
//! With the SQLite backend, a connection is a single database file. This
//! module holds the file-level helpers used by the New / Open / Save /
//! Save As handlers and by the startup check: dialog filters, path
//! derivation and checks, and the SQLite checkpoint and copy operations.

use std::path::Path;

use anyhow::Context as _;
use tax_db_sqlite::SqliteRepository;

use crate::file_dialogs::owned_filters;

/// Label shown for the database file type in the connection dialogs.
const DB_FILTER_LABEL: &str = "Tax Estimator Database";

/// Extensions offered (and filtered on) in the connection dialogs.
const DB_EXTENSIONS: [&str; 3] = ["db", "sqlite", "sqlite3"];

/// Filename pre-filled when creating a brand-new database.
pub const DEFAULT_DATABASE_FILE_NAME: &str = "taxes.db";

/// The database file filters used by every connection dialog.
pub fn db_file_filters() -> Vec<(String, Vec<String>)> {
    owned_filters(&[(DB_FILTER_LABEL, DB_EXTENSIONS.as_slice())])
}

/// Folder the connection dialogs should open in: the directory holding
/// `database_url`, falling back to the working directory.
pub fn connection_dialog_directory(database_url: &str) -> String {
    Path::new(database_url)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string())
}

/// File name of the database at `database_url`, used to pre-fill *Save As*.
/// Falls back to [`DEFAULT_DATABASE_FILE_NAME`] when the URL has no file name.
pub fn connection_file_name(database_url: &str) -> String {
    Path::new(database_url)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_DATABASE_FILE_NAME.to_string())
}

/// True when `candidate` resolves to the same existing file as `current`. A
/// path that does not yet exist can never collide, so this returns `false`.
pub fn is_same_file(
    current: &str,
    candidate: &Path,
) -> bool {
    match (
        std::fs::canonicalize(current),
        std::fs::canonicalize(candidate),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Opens `url` and collapses its write-ahead log into the main file.
pub async fn checkpoint_database(url: &str) -> anyhow::Result<()> {
    let repo = SqliteRepository::new(url).await?;
    repo.checkpoint().await
}

/// Writes a standalone copy of the database at `source` to `target`,
/// replacing `target` if it already exists.
pub async fn copy_database(
    source: &str,
    target: &Path,
) -> anyhow::Result<()> {
    if target.exists() {
        // rfd already confirmed the overwrite, and `VACUUM INTO`
        // refuses a pre-existing file.
        std::fs::remove_file(target)
            .with_context(|| format!("Could not overwrite '{}'", target.display()))?;
    }
    let repo = SqliteRepository::new(source).await?;
    repo.backup_to(target).await
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn db_file_filters_offers_every_database_extension() {
        let filters = db_file_filters();
        assert_eq!(filters.len(), 1);

        let (label, extensions) = &filters[0];
        assert_eq!(label, DB_FILTER_LABEL);
        assert_eq!(
            extensions,
            &vec![
                "db".to_string(),
                "sqlite".to_string(),
                "sqlite3".to_string()
            ]
        );
    }

    #[test]
    fn connection_dialog_directory_uses_parent_of_database_file() {
        assert_eq!(
            connection_dialog_directory("/data/taxes/current.db"),
            "/data/taxes"
        );
    }

    #[test]
    fn connection_dialog_directory_falls_back_to_working_directory() {
        assert_eq!(connection_dialog_directory("current.db"), ".");
        assert_eq!(connection_dialog_directory(""), ".");
    }

    #[test]
    fn connection_file_name_uses_database_file_name() {
        assert_eq!(connection_file_name("/data/taxes/current.db"), "current.db");
    }

    #[test]
    fn connection_file_name_falls_back_to_default() {
        assert_eq!(connection_file_name(""), DEFAULT_DATABASE_FILE_NAME);
    }

    #[test]
    fn is_same_file_is_false_when_candidate_does_not_exist() {
        let missing = std::env::temp_dir().join("tax-ui-connection-missing-file.db");
        assert!(!is_same_file("also-missing.db", &missing));
    }

    #[test]
    fn is_same_file_is_true_for_the_same_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "tax-ui-connection-same-file-{}.db",
            std::process::id()
        ));
        std::fs::write(&path, b"").expect("temp file should be writable");

        let same = is_same_file(&path.to_string_lossy(), &path);
        let _ = std::fs::remove_file(&path);

        assert!(same);
    }
}
