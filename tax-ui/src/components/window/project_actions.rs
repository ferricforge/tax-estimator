//! *New Project*, *Open Project*, *Save*, and *Save As* handlers for
//! [`AppWindow`], plus the async helpers that report their results back to
//! the window. File-level work is delegated to [`crate::project`].

use gpui::{AnyWindowHandle, App, AsyncApp, Context, WeakEntity, Window};
use tax_core::db::DbConfig;

use super::AppWindow;
use crate::components::{ErrorDialog, show_err};
use crate::config::AppConfig;
use crate::file_dialogs::{get_file_path, put_file_path};
use crate::project::{
    DEFAULT_PROJECT_FILE_NAME, checkpoint_database, copy_database, db_file_filters, is_same_file,
    project_dialog_directory, project_file_name,
};
use crate::session::{DatabaseTarget, switch_database};
use crate::state::ActiveTaxYear;

/// What the window should do after [`apply_project_switch`] repoints the
/// application at a different database file.
#[derive(Clone, Copy)]
enum ProjectSwitch {
    /// A fresh file (*New Project*) — clear the estimate form.
    Created,
    /// An existing file (*Open Project*) — clear the estimate form.
    Opened,
    /// A copy of the current data (*Save As*) — keep the form and just
    /// reload the active tax year.
    Branched,
}

impl ProjectSwitch {
    fn status_verb(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Opened => "Opened",
            Self::Branched => "Saved a copy to",
        }
    }

    fn clears_form(self) -> bool {
        !matches!(self, Self::Branched)
    }
}

impl AppWindow {
    /// Prompts for a new database file and switches to it. The SQLite factory
    /// creates the file, runs migrations, and seeds the reference tax data.
    pub(super) fn handle_new_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = project_dialog_directory(&AppConfig::get(cx).database_url);
        let target = DatabaseTarget::from_config(cx);
        let filters = db_file_filters();
        let window_handle = window.window_handle();

        cx.spawn(async move |this, async_cx| {
            let Some(path) =
                put_file_path(directory, DEFAULT_PROJECT_FILE_NAME.to_string(), filters).await
            else {
                tracing::info!("New project cancelled");
                return;
            };

            if path.exists() {
                let _ = window_handle.update(async_cx, |_, window, cx| {
                    ErrorDialog::show(
                        "File already exists",
                        &[format!(
                            "'{}' already exists. Use Open Project to open it instead.",
                            path.display()
                        )],
                        window,
                        cx,
                    );
                });
                return;
            }

            let db_config = target.db_config(path.to_string_lossy().into_owned());

            apply_project_switch(
                this,
                window_handle,
                async_cx,
                db_config,
                ProjectSwitch::Created,
            )
            .await;
        })
        .detach();
    }

    /// Prompts for an existing database file and switches to it.
    pub(super) fn handle_open_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = project_dialog_directory(&AppConfig::get(cx).database_url);
        let target = DatabaseTarget::from_config(cx);
        let filters = db_file_filters();
        let window_handle = window.window_handle();

        cx.spawn(async move |this, async_cx| {
            let Some(path) = get_file_path(directory, filters).await else {
                tracing::info!("Open project cancelled");
                return;
            };

            let db_config = target.db_config(path.to_string_lossy().into_owned());

            apply_project_switch(
                this,
                window_handle,
                async_cx,
                db_config,
                ProjectSwitch::Opened,
            )
            .await;
        })
        .detach();
    }

    /// Flushes the active database's write-ahead log into its main file.
    ///
    /// Estimates are written as they are calculated, so this saves nothing
    /// new; it just leaves the on-disk `.db` self-contained.
    pub(super) fn handle_save_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = AppConfig::get(cx).database_url.clone();
        let window_handle = window.window_handle();

        self.set_status("Saving…");
        cx.notify();

        cx.spawn(async move |this, async_cx| {
            let result = checkpoint_database(&source).await;
            report_save_result(&this, window_handle, async_cx, &source, result);
        })
        .detach();
    }

    /// Prompts for a destination, writes a standalone copy of the current
    /// project there, and makes that copy the active project.
    pub(super) fn handle_save_project_as(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = AppConfig::get(cx).database_url.clone();
        let target = DatabaseTarget::from_config(cx);
        let directory = project_dialog_directory(&source);
        let default_name = project_file_name(&source);
        let filters = db_file_filters();
        let window_handle = window.window_handle();

        cx.spawn(async move |this, async_cx| {
            let Some(destination) = put_file_path(directory, default_name, filters).await else {
                tracing::info!("Save As cancelled");
                return;
            };

            if is_same_file(&source, &destination) {
                // Saving over the current project is just a plain Save.
                let result = checkpoint_database(&source).await;
                report_save_result(&this, window_handle, async_cx, &source, result);
                return;
            }

            if let Err(e) = copy_database(&source, &destination).await {
                let e = e.context("Could not save a copy of the project");
                tracing::error!(error = ?e, "save as failed");
                show_err(window_handle, async_cx, "Save As failed", &e);
                return;
            }

            let db_config = target.db_config(destination.to_string_lossy().into_owned());

            apply_project_switch(
                this,
                window_handle,
                async_cx,
                db_config,
                ProjectSwitch::Branched,
            )
            .await;
        })
        .detach();
    }

    /// Re-loads the tax-year configuration for the year currently in the
    /// form. Used after a *Save As*, whose copy keeps the form values even
    /// though [`switch_database`] has cleared the cached [`ActiveTaxYear`].
    fn reload_active_year(
        &self,
        cx: &mut App,
    ) {
        let year = self.form.read(cx).tax_year(cx);
        if let Some(year) = year {
            ActiveTaxYear::load(year, cx);
        }
    }
}

/// Rebuilds the repository against `db_config`, then refreshes the window:
/// either clears the estimate form (a different project is now open) or keeps
/// it and reloads the active tax year (the data was merely copied).
async fn apply_project_switch(
    this: WeakEntity<AppWindow>,
    window_handle: AnyWindowHandle,
    async_cx: &mut AsyncApp,
    db_config: DbConfig,
    outcome: ProjectSwitch,
) {
    let project_path = db_config.connection_string.clone();

    if let Err(e) = switch_database(async_cx, db_config).await {
        let e = e.context(format!("Could not open project '{project_path}'"));
        tracing::error!(error = ?e, "project switch failed");
        show_err(window_handle, async_cx, "Open failed", &e);
        restore_ready_status(&this, window_handle, async_cx);
        return;
    }

    let refreshed = window_handle.update(async_cx, |_, window, cx| {
        let _ = this.update(cx, |app_window, view_cx| {
            if outcome.clears_form() {
                app_window.form.update(view_cx, |form, form_cx| {
                    form.reset(window, form_cx);
                });
            } else {
                app_window.reload_active_year(view_cx);
            }
            app_window.set_status(format!("{} {project_path}", outcome.status_verb()));
            view_cx.notify();
        });
    });

    if refreshed.is_err() {
        tracing::debug!(%project_path, "window closed before project switch finished");
    }
}

/// Reports the result of a *Save* (or a same-file *Save As*) to the user.
fn report_save_result(
    this: &WeakEntity<AppWindow>,
    window_handle: AnyWindowHandle,
    async_cx: &mut AsyncApp,
    database: &str,
    result: anyhow::Result<()>,
) {
    match result {
        Ok(()) => {
            tracing::info!(%database, "flushed database to disk");
            let message = format!("Saved {database}");
            let _ = window_handle.update(async_cx, |_, _window, cx| {
                let _ = this.update(cx, |app_window, view_cx| {
                    app_window.set_status(message);
                    view_cx.notify();
                });
            });
        }
        Err(e) => {
            let e = e.context("Could not flush the database to disk");
            tracing::error!(error = ?e, "save failed");
            show_err(window_handle, async_cx, "Save failed", &e);
            restore_ready_status(this, window_handle, async_cx);
        }
    }
}

/// Resets the status line to "Ready" after a failed project operation.
fn restore_ready_status(
    this: &WeakEntity<AppWindow>,
    window_handle: AnyWindowHandle,
    async_cx: &mut AsyncApp,
) {
    let _ = window_handle.update(async_cx, |_, _window, cx| {
        let _ = this.update(cx, |app_window, view_cx| {
            app_window.set_status("Ready");
            view_cx.notify();
        });
    });
}
