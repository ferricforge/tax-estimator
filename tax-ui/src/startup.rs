//! Startup check for the configured database.
//!
//! Opening a SQLite database that does not exist silently creates an empty
//! one. [`open_database_or_ask`] checks first and, when the database is
//! missing, asks whether to open an existing database or create a new one.
//! No window exists yet at that point, so native message boxes are used.

use std::path::Path;

use anyhow::Result;
use gpui::AsyncApp;
use rfd::{AsyncMessageDialog, MessageButtons, MessageDialogResult, MessageLevel};

use crate::connection::{DEFAULT_DATABASE_FILE_NAME, connection_dialog_directory, db_file_filters};
use crate::file_dialogs::{get_file_path, put_file_path};
use crate::session::{DatabaseTarget, init_database, missing_configured_database, switch_database};

/// Label of the button that opens an existing database.
const OPEN_LABEL: &str = "Open…";

/// Label of the button that creates a new database.
const NEW_LABEL: &str = "New…";

/// Label of the button that closes the application.
const QUIT_LABEL: &str = "Quit";

/// Result of [`open_database_or_ask`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupOutcome {
    /// A database is open and the main window can be shown.
    Ready,
    /// The user chose to quit instead of selecting a database.
    Declined,
}

/// What the user chose in the "database not found" prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Open,
    New,
    Quit,
}

/// Opens the configured database. When it does not exist, asks the user to
/// open or create one instead, repeating until a database opens or the user
/// quits.
pub async fn open_database_or_ask(cx: &mut AsyncApp) -> Result<StartupOutcome> {
    let Some(missing) = cx.update(|cx| missing_configured_database(cx))? else {
        init_database(cx).await?;
        return Ok(StartupOutcome::Ready);
    };

    tracing::warn!(database = %missing, "configured database not found; asking what to do");
    let target = cx.update(|cx| DatabaseTarget::from_config(cx))?;
    let directory = dialog_directory(&missing);

    loop {
        let path = match ask_what_to_do(&missing).await {
            Choice::Quit => return Ok(StartupOutcome::Declined),
            Choice::Open => {
                let picked = get_file_path(directory.clone(), db_file_filters()).await;
                match picked {
                    Some(path) => path,
                    None => continue,
                }
            }
            Choice::New => {
                let picked = put_file_path(
                    directory.clone(),
                    DEFAULT_DATABASE_FILE_NAME.to_string(),
                    db_file_filters(),
                )
                .await;
                let path = match picked {
                    Some(path) => path,
                    None => continue,
                };

                if path.exists() {
                    let description = format!(
                        "'{}' already exists. Choose Open to use it instead.",
                        path.display()
                    );
                    tell("File already exists", description).await;
                    continue;
                }
                path
            }
        };

        let db_config = target.db_config(path.to_string_lossy().into_owned());
        match switch_database(cx, db_config).await {
            Ok(()) => return Ok(StartupOutcome::Ready),
            Err(error) => {
                tracing::error!(error = ?error, "could not open the selected database");
                tell("Open failed", format!("{error:#}")).await;
            }
        }
    }
}

/// Folder the file dialogs open in: the folder of the missing database, or
/// the working directory when that folder does not exist either.
fn dialog_directory(missing: &str) -> String {
    let directory = connection_dialog_directory(missing);
    if Path::new(&directory).is_dir() {
        directory
    } else {
        ".".to_string()
    }
}

/// Shows the "database not found" prompt.
async fn ask_what_to_do(missing: &str) -> Choice {
    let description = format!(
        "The database '{missing}' could not be found.\n\n\
         Open an existing database or create a new one."
    );
    let result = AsyncMessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Database not found")
        .set_description(description)
        .set_buttons(MessageButtons::YesNoCancelCustom(
            OPEN_LABEL.to_string(),
            NEW_LABEL.to_string(),
            QUIT_LABEL.to_string(),
        ))
        .show()
        .await;

    match result {
        MessageDialogResult::Yes => Choice::Open,
        MessageDialogResult::No => Choice::New,
        MessageDialogResult::Custom(label) if label == OPEN_LABEL => Choice::Open,
        MessageDialogResult::Custom(label) if label == NEW_LABEL => Choice::New,
        _ => Choice::Quit,
    }
}

/// Shows an error message with a single OK button.
async fn tell(
    title: &str,
    description: impl Into<String>,
) {
    let _ = AsyncMessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(title)
        .set_description(description)
        .set_buttons(MessageButtons::Ok)
        .show()
        .await;
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::dialog_directory;

    #[test]
    fn dialog_directory_falls_back_when_the_folder_is_missing() {
        assert_eq!(dialog_directory("/no/such/folder/taxes.db"), ".");
    }

    #[test]
    fn dialog_directory_uses_an_existing_folder() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let database = dir.path().join("taxes.db");
        let database = database.to_string_lossy();

        assert_eq!(
            dialog_directory(&database),
            dir.path().to_string_lossy().into_owned()
        );
    }
}
