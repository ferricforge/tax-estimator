//! Native file dialogs (through `rfd`) and the filter helpers they share.

use std::path::PathBuf;

use rfd::AsyncFileDialog;

/// Convert borrowed filter definitions into owned `String` values.
///
/// This is useful when filter data needs to be moved into an `async move`
/// closure or any `'static` context where references cannot be used.
pub fn owned_filters(filters: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    filters
        .iter()
        .map(|(name, exts)| {
            (
                name.to_string(),
                exts.iter().map(|e| e.to_string()).collect(),
            )
        })
        .collect()
}

/// Opens an async file picker dialog with the given filters and starting directory.
///
/// Each filter is a `(name, extensions)` pair, e.g. `("Excel", &["xlsx", "xlsm"])`.
pub async fn get_file_path(
    location: String,
    filters: Vec<(String, Vec<String>)>,
) -> Option<PathBuf> {
    let dialog = with_filters(AsyncFileDialog::new().set_directory(&location), &filters);

    let file = dialog.pick_file().await?;
    Some(file.path().to_path_buf())
}

/// Opens an async *save* dialog, returning the chosen path.
///
/// `default_name` pre-fills the filename field. Filters use the same
/// `(name, extensions)` shape as [`get_file_path`].
pub async fn put_file_path(
    location: String,
    default_name: String,
    filters: Vec<(String, Vec<String>)>,
) -> Option<PathBuf> {
    let dialog = AsyncFileDialog::new()
        .set_directory(&location)
        .set_file_name(&default_name);
    let dialog = with_filters(dialog, &filters);

    let file = dialog.save_file().await?;
    Some(file.path().to_path_buf())
}

/// Adds each `(name, extensions)` filter to `dialog`.
fn with_filters(
    mut dialog: AsyncFileDialog,
    filters: &[(String, Vec<String>)],
) -> AsyncFileDialog {
    for (name, extensions) in filters {
        let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(name, &ext_refs);
    }
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_owned_filter_one_extension() {
        let input_filter = &[("Excel", &["xlsx"] as &[_])];
        let expected = vec![("Excel".to_string(), vec!["xlsx".to_string()])];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }

    #[test]
    fn test_owned_filter_multiple_extensions() {
        let input_filter = &[("Excel", &["xlsx", "xlsm", "xlsb"] as &[_])];
        let expected = vec![(
            "Excel".to_string(),
            vec!["xlsx".to_string(), "xlsm".to_string(), "xlsb".to_string()],
        )];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }

    #[test]
    fn test_owned_filter_multiple_types() {
        let input_filter = &[
            ("Excel", &["xlsx"] as &[_]),
            ("SQLite", &["db", "db3"] as &[_]),
        ];
        let expected = vec![
            ("Excel".to_string(), vec!["xlsx".to_string()]),
            (
                "SQLite".to_string(),
                vec!["db".to_string(), "db3".to_string()],
            ),
        ];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }
}
