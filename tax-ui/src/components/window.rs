// components

use std::path::Path;
use std::rc::Rc;

use anyhow::Context as _;
use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement, Render, Styled, Subscription, WeakEntity,
    Window, div, px,
};
use gpui_component::{Root, StyledExt, WindowExt, v_flex};
use tax_core::db::DbConfig;
use tax_db_sqlite::SqliteRepository;
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "macos"))]
use crate::components::build_menu_bar;
use crate::components::estimate_selector::OnSelectEstimate;
use crate::components::file_picker::{get_file_path, put_file_path};
use crate::components::{
    ErrorDialog, EstimateSelector, EstimatedIncomeForm, InfoDialog, LoadEstimate, NewProject,
    OpenProject, SaveProject, SaveProjectAs, SeWorksheetForm, show_err,
};
use crate::config::AppConfig;
#[cfg(not(target_os = "linux"))]
use crate::quit;
use crate::repository::{ActiveTaxYear, TaxRepo, switch_repository};

/// Label shown for the database file type in the project dialogs.
const DB_FILTER_LABEL: &str = "Tax Estimator Database";

/// Extensions offered (and filtered on) in the project dialogs.
const DB_EXTENSIONS: [&str; 3] = ["db", "sqlite", "sqlite3"];

/// Filename pre-filled when creating a brand-new project.
const DEFAULT_PROJECT_FILE_NAME: &str = "taxes.db";

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

pub struct AppWindow {
    _window_close_subscription: Subscription,
    focus_handle: FocusHandle,
    status_message: Option<String>,
    form: Entity<EstimatedIncomeForm>,
}

impl AppWindow {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.on_window_closed(|_cx: &mut App| {
            info!("Window closed callback");
            #[cfg(not(target_os = "linux"))]
            quit(&Quit, _cx);
        });

        // Actions are dispatched along the focus path, so the root element
        // must be focused (or an ancestor of the focused element) for its
        // `on_action` handlers to run. Focus it up front so menu actions work
        // before the user has clicked into any field.
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let worksheet = cx.new(|form_cx| SeWorksheetForm::new(window, form_cx));
        let form = cx.new(|form_cx| EstimatedIncomeForm::new(worksheet.clone(), window, form_cx));

        info!("Window constructed");
        Self {
            _window_close_subscription: subscription,
            focus_handle,
            status_message: None,
            form,
        }
    }

    /// Fetches saved estimates and opens a selector dialog.
    fn handle_load_estimate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = TaxRepo::try_get(cx) else {
            tracing::warn!("TaxRepo not initialised; cannot load estimates");
            ErrorDialog::show(
                "Cannot load estimates",
                &["The database connection is not available.".to_string()],
                window,
                cx,
            );
            return;
        };

        tracing::info!("Loading saved estimates");
        let window_handle = window.window_handle();

        cx.spawn(
            async move |this, async_cx| match repo.list_estimates(None).await {
                Ok(estimates) if estimates.is_empty() => {
                    tracing::info!("No saved estimates found");
                    let _ = window_handle.update(async_cx, |_, window, cx| {
                        InfoDialog::show(
                            "No saved estimates",
                            "There are no saved estimates to load yet. Calculate an estimate \
                             and it will be saved automatically.",
                            window,
                            cx,
                        );
                    });
                }
                Ok(estimates) => {
                    tracing::info!("Found {} saved estimate(s)", estimates.len());
                    for estimate in &estimates {
                        tracing::info!("{}", estimate);
                    }
                    let _ = window_handle.update(async_cx, |_, window, cx| {
                        let _ = this.update(cx, move |app_window, view_cx| {
                            let mut estimates_opt = Some(estimates);
                            let form = app_window.form.clone();
                            let on_select: OnSelectEstimate =
                                Rc::new(move |estimate, window, cx| {
                                    tracing::info!("Selected estimate: {}", estimate);
                                    form.update(cx, |form, form_cx| {
                                        form.populate_from_estimate(estimate, window, form_cx);
                                    });
                                });
                            let selector = view_cx.new(|sel_cx| {
                                EstimateSelector::new(
                                    estimates_opt.take().unwrap(),
                                    on_select,
                                    window,
                                    sel_cx,
                                )
                            });
                            window.open_dialog(view_cx, move |dialog, _w, _cx| {
                                dialog
                                    .title("Load Saved Estimate")
                                    .w(px(500.0))
                                    .child(selector.clone())
                            });
                        });
                    });
                }
                Err(e) => {
                    let e = anyhow::Error::from(e).context("Could not load saved estimates");
                    tracing::error!(error = ?e, "Failed to load estimates");
                    show_err(window_handle, async_cx, "Load failed", &e);
                }
            },
        )
        .detach();
    }

    fn set_status(
        &mut self,
        message: impl Into<String>,
    ) {
        self.status_message = Some(message.into());
    }

    /// Re-loads the tax-year configuration for the year currently in the
    /// form. Used after a *Save As*, whose copy keeps the form values even
    /// though [`switch_repository`] has cleared the cached [`ActiveTaxYear`].
    fn reload_active_year(
        &self,
        cx: &mut App,
    ) {
        let year = self.form.read(cx).tax_year(cx);
        if let Some(year) = year {
            ActiveTaxYear::load(year, cx);
        }
    }

    /// Prompts for a new database file and switches to it. The SQLite factory
    /// creates the file, runs migrations, and seeds the reference tax data.
    fn handle_new_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = project_dialog_directory(cx);
        let backend = AppConfig::get(cx).database_backend.as_str().to_string();
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

            let db_config = DbConfig {
                backend,
                connection_string: path.to_string_lossy().into_owned(),
            };
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
    fn handle_open_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = project_dialog_directory(cx);
        let backend = AppConfig::get(cx).database_backend.as_str().to_string();
        let filters = db_file_filters();
        let window_handle = window.window_handle();

        cx.spawn(async move |this, async_cx| {
            let Some(path) = get_file_path(directory, filters).await else {
                tracing::info!("Open project cancelled");
                return;
            };

            let db_config = DbConfig {
                backend,
                connection_string: path.to_string_lossy().into_owned(),
            };
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
    fn handle_save_project(
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
    fn handle_save_project_as(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = AppConfig::get(cx).database_url.clone();
        let backend = AppConfig::get(cx).database_backend.as_str().to_string();
        let directory = project_dialog_directory(cx);
        let default_name = Path::new(&source)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| DEFAULT_PROJECT_FILE_NAME.to_string());
        let filters = db_file_filters();
        let window_handle = window.window_handle();

        cx.spawn(async move |this, async_cx| {
            let Some(target) = put_file_path(directory, default_name, filters).await else {
                tracing::info!("Save As cancelled");
                return;
            };

            if is_same_file(&source, &target) {
                // Saving over the current project is just a plain Save.
                let result = checkpoint_database(&source).await;
                report_save_result(&this, window_handle, async_cx, &source, result);
                return;
            }

            let copied: anyhow::Result<()> = async {
                if target.exists() {
                    // rfd already confirmed the overwrite, and `VACUUM INTO`
                    // refuses a pre-existing file.
                    std::fs::remove_file(&target)
                        .with_context(|| format!("Could not overwrite '{}'", target.display()))?;
                }
                let repo = SqliteRepository::new(&source).await?;
                repo.backup_to(&target).await
            }
            .await;

            if let Err(e) = copied {
                let e = e.context("Could not save a copy of the project");
                tracing::error!(error = ?e, "save as failed");
                show_err(window_handle, async_cx, "Save As failed", &e);
                return;
            }

            let db_config = DbConfig {
                backend,
                connection_string: target.to_string_lossy().into_owned(),
            };
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

    fn main_body(&self) -> impl IntoElement {
        v_flex().size_full().p_5().gap_4().child(self.form.clone())
    }

    fn render_body(&self) -> impl IntoElement {
        let root = {
            let base = v_flex().size_full().gap_0();
            #[cfg(not(target_os = "macos"))]
            {
                base.child(build_menu_bar())
            }
            #[cfg(target_os = "macos")]
            {
                base
            }
        };

        root.child(self.main_body())
    }

    fn render_status_bar(&self) -> impl IntoElement {
        let status_text = self
            .status_message
            .clone()
            .unwrap_or_else(|| "Ready".to_string());

        div()
            .w_full()
            .px_3()
            .py_2()
            .border_t_1()
            .child(div().text_size(px(11.0)).child(status_text))
    }
}

impl Render for AppWindow {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("app-window")
            .key_context("AppWindow")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &LoadEstimate, window, cx| {
                this.handle_load_estimate(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewProject, window, cx| {
                this.handle_new_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenProject, window, cx| {
                this.handle_open_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveProject, window, cx| {
                this.handle_save_project(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveProjectAs, window, cx| {
                this.handle_save_project_as(window, cx);
            }))
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .child(self.render_body())
            .child(self.render_status_bar())
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

/// The database file filters used by every project dialog.
fn db_file_filters() -> Vec<(String, Vec<String>)> {
    vec![(
        DB_FILTER_LABEL.to_string(),
        DB_EXTENSIONS.iter().map(|ext| ext.to_string()).collect(),
    )]
}

/// Folder the project dialogs should open in: the directory holding the
/// current database, falling back to the working directory.
fn project_dialog_directory(cx: &App) -> String {
    Path::new(&AppConfig::get(cx).database_url)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string())
}

/// True when `candidate` resolves to the same existing file as `current`. A
/// path that does not yet exist can never collide, so this returns `false`.
fn is_same_file(
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
async fn checkpoint_database(url: &str) -> anyhow::Result<()> {
    let repo = SqliteRepository::new(url).await?;
    repo.checkpoint().await
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

    if let Err(e) = switch_repository(async_cx, db_config).await {
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

impl Focusable for AppWindow {
    fn focus_handle(
        &self,
        _cx: &App,
    ) -> FocusHandle {
        self.focus_handle.clone()
    }
}
