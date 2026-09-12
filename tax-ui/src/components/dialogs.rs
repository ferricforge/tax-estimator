use gpui::{App, PromptLevel, Window};

/// Opens a native prompt with a single **OK** button.
///
/// `Window::prompt` returns a receiver for the user's choice. With only
/// one button there is nothing to wait for, so the receiver is dropped
/// explicitly; this does not cancel the prompt.
fn prompt_ok(
    level: PromptLevel,
    title: &str,
    message: &str,
    window: &mut Window,
    cx: &mut App,
) {
    drop(window.prompt(level, title, Some(message), &["OK"], cx));
}

/// ErrorDialog displays a warning dialog with a list of errors.
pub struct ErrorDialog;

impl ErrorDialog {
    /// Show a warning dialog with a list of error messages.
    pub fn show(
        title: &str,
        errors: &[String],
        window: &mut Window,
        cx: &mut App,
    ) {
        let detail = Self::format_error_list(errors);
        prompt_ok(PromptLevel::Warning, title, &detail, window, cx);
    }

    /// Show a warning dialog for an [`anyhow::Error`], listing each link in
    /// its context chain (outermost first) as a separate line.
    pub fn show_error(
        title: &str,
        error: &anyhow::Error,
        window: &mut Window,
        cx: &mut App,
    ) {
        let lines: Vec<String> = error.chain().map(ToString::to_string).collect();
        Self::show(title, &lines, window, cx);
    }

    fn format_error_list(errors: &[String]) -> String {
        match errors.len() {
            0 => "An unknown error occurred.".to_owned(),
            1 => errors[0].clone(),
            _ => errors
                .iter()
                .map(|e| format!("• {e}"))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// InfoDialog displays an informational dialog with a single message.
pub struct InfoDialog;

impl InfoDialog {
    /// Show an informational dialog with a single message.
    pub fn show(
        title: &str,
        message: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        prompt_ok(PromptLevel::Info, title, message, window, cx);
    }
}
