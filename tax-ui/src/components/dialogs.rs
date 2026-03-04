use gpui::{App, PromptLevel, Window};

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
        let _x = window.prompt(PromptLevel::Warning, title, Some(&detail), &["OK"], cx);
        // `prompt()` returns a future but we don't really need this to be async
        // so we drop it here to avoid accumulating futues.
        std::mem::drop(_x);
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
