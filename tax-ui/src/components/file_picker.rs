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
