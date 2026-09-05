#![allow(unused)]
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
pub async fn get_folder_path(location: String) -> Option<PathBuf> {
    let dialog = AsyncFileDialog::new().set_directory(&location);

    let folder = dialog.pick_folder().await?;
    Some(folder.path().to_path_buf())
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
    let mut dialog = AsyncFileDialog::new()
        .set_directory(&location)
        .set_file_name(&default_name);

    for (name, extensions) in &filters {
        let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();
        dialog = dialog.add_filter(name, &ext_refs);
    }

    let file = dialog.save_file().await?;
    Some(file.path().to_path_buf())
}
