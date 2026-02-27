use std::path::PathBuf;

use gpui::{
    App, AppContext, ClickEvent, Context, Div, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, TextAlign, Window, div, px,
};
use gpui_component::{
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use tracing::debug;

use crate::{
    components::{
        dialogs::{get_file_path, get_folder_path, owned_filters},
        make_button,
    },
    logging::log_task_error,
    models::FileFormModel,
};

pub struct FileSelectionForm {
    source_file: Entity<InputState>,
    database_file: Entity<InputState>,
    log_directory: Entity<InputState>,
    log_stdout: bool,
}

impl FileSelectionForm {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let source_file = make_input_state("Source file path...", window, cx);
        let database_file = make_input_state("Database file path...", window, cx);
        let log_file = make_input_state("Log folder path...", window, cx);

        Self {
            source_file,
            database_file,
            log_directory: log_file,
            log_stdout: false,
        }
    }

    /// Collects the current form values into a [`FileFormModel`].
    pub fn to_model(
        &self,
        cx: &App,
    ) -> FileFormModel {
        FileFormModel {
            source_file: PathBuf::from(self.source_file.read(cx).value().as_str().trim()),
            database_file: PathBuf::from(self.database_file.read(cx).value().as_str().trim()),
            log_directory: PathBuf::from(self.log_directory.read(cx).value().as_str().trim()),
            log_stdout: self.log_stdout,
        }
    }

    /// Returns the source file input state.
    pub fn source_file(&self) -> &Entity<InputState> {
        &self.source_file
    }

    /// Returns the database file input state.
    pub fn database_file(&self) -> &Entity<InputState> {
        &self.database_file
    }

    /// Returns the log director input state
    pub fn log_folder(&self) -> &Entity<InputState> {
        &self.log_directory
    }

    /// Returns whether output should be logged to stdout.
    pub fn log_stdout(&self) -> bool {
        self.log_stdout
    }
}

impl Render for FileSelectionForm {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_2()
            .size_full()
            .child(make_input_row(
                &self.source_file,
                "Source File:",
                "source-select",
                "Select File",
                file_select_handler(
                    &self.source_file,
                    "~/Desktop",
                    &[
                        ("Excel", &["xlsx", "xlsm"] as &[_]),
                        ("CSV", &["csv"] as &[_]),
                    ],
                    false,
                ),
            ))
            .child(make_input_row(
                &self.database_file,
                "Database:",
                "db-select",
                "Select Database",
                file_select_handler(
                    &self.database_file,
                    "~/Desktop",
                    &[("SQLite", &["db", "db3", "sqlite"] as &[_])],
                    false,
                ),
            ))
            .child(make_input_row(
                &self.log_directory,
                "Log Folder:",
                "log-select",
                "Select Log Folder",
                file_select_handler(&self.log_directory, "~/Desktop", &[], true),
            ))
            .child(
                v_flex().gap_4().p_5().child(
                    Checkbox::new("log-checkbox")
                        .label("Log to stdout")
                        .border_2()
                        .checked(self.log_stdout)
                        .on_click(cx.listener(|view, checked, _, cx| {
                            view.log_stdout = *checked;
                            cx.notify();
                        })),
                ),
            )
    }
}

fn make_input_state(
    label: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<FileSelectionForm>,
) -> Entity<InputState> {
    cx.new(|closure_cx| InputState::new(window, closure_cx).placeholder(label.into()))
}

fn make_input_row(
    state: &Entity<InputState>,
    input_label: impl Into<SharedString>,
    button_id: impl Into<SharedString>,
    button_label: impl Into<SharedString>,
    button_callback: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    make_labeled_row(input_label)
        .child(Input::new(state).flex_grow())
        .child(make_button(button_id, button_label, button_callback))
}

/// Creates the common outer container and label used by both input and select
/// rows, ensuring consistent alignment, spacing, and border styling.
fn make_labeled_row(label: impl Into<SharedString>) -> Div {
    h_flex()
        .items_center()
        .gap_5()
        .p(px(2.))
        .rounded_md()
        .border_1()
        .child(
            div()
                .min_w(px(100.))
                .text_align(TextAlign::Right)
                .child(label.into()),
        )
}

/// Creates a click handler that opens an async file dialog and populates the
/// given input field with the selected path.
///
/// The outer closure captures owned copies of `input`, `directory`, and
/// `filters`. Each click then clones these into an async task that runs
/// the file dialog off the main thread and writes back via `async_window`.
fn file_select_handler(
    input: &Entity<InputState>,
    directory: &str,
    filters: &[(&str, &[&str])],
    select_dir: bool,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let input = input.clone();
    let directory = directory.to_string();
    let filters = owned_filters(filters);

    move |_, window, cx| {
        let input = input.clone();
        let filters = filters.clone();
        let directory = directory.clone();
        let select_dir = select_dir;
        let mut async_window = window.to_async(cx);
        cx.spawn(async move |_async_cx| {
            let result: anyhow::Result<()> = async {
                let path = if select_dir {
                    get_folder_path(directory).await
                } else {
                    get_file_path(directory, filters).await
                };
                if let Some(path) = path {
                    let path_str = path.display().to_string();
                    async_window.update(|window, cx| {
                        input.update(cx, |state, cx| {
                            state.set_value(path_str, window, cx);
                        });
                    })?;
                } else {
                    debug!("No file/folder selected");
                }

                Ok(())
            }
            .await;

            log_task_error("file_select_handler", result);
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}
