use rfd::AsyncFileDialog;
use std::path::PathBuf;

/// Opens an async file picker dialog with the given filters and starting directory.
///
/// Each filter is a `(name, extensions)` pair, e.g. `("Excel", &["xlsx", "xlsm"])`.
pub async fn get_file_path(
    location: String,
    filters: Vec<(String, Vec<String>)>,
) -> Option<PathBuf> {
    let mut dialog = AsyncFileDialog::new().set_directory(&location);

    for (name, extensions) in &filters {
        let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(name, &ext_refs);
    }

    let file = dialog.pick_file().await?;
    Some(file.path().to_path_buf())
}

/// Opens an async file picker dialog to select a directory.
///
pub async fn get_folder_path(location: String) -> Option<PathBuf> {
    let dialog = AsyncFileDialog::new().set_directory(&location);

    let folder = dialog.pick_folder().await?;
    Some(folder.path().to_path_buf())
}

/// Converts borrowed filter definitions into owned `String` values.
///
/// This is useful when filter data needs to be moved into an `async move`
/// closure or other `'static` context where references cannot be used.
///
/// Each entry is a `(name, extensions)` pair suitable for passing to
/// [`get_file_path`].
///
/// # Examples
///
/// ```
/// use gpui_demo::components::owned_filters;
///
/// let filters = owned_filters(&[
///     ("Excel", &["xlsx", "xlsm"]),
///     ("CSV", &["csv"]),
/// ]);
///
/// assert_eq!(filters.len(), 2);
/// assert_eq!(filters[0].0, "Excel");
/// assert_eq!(filters[0].1, vec!["xlsx", "xlsm"]);
/// assert_eq!(filters[1].0, "CSV");
/// assert_eq!(filters[1].1, vec!["csv"]);
/// ```
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
