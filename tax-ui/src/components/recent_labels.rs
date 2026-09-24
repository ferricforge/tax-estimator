//! Menu labels for the recent connections.
//!
//! A label is the connection's path. A long path is shortened by replacing
//! segments in the middle with an ellipsis. The file name and its folder are
//! always kept, and as much of both ends of the path as fits is kept with
//! them, so entries with the same file name can be told apart.

use crate::config::RecentConnection;

/// Longest label, in characters, before a path is shortened.
const MAX_LABEL_CHARS: usize = 48;

/// Stands in for the segments left out of a shortened path.
const ELLIPSIS: &str = "…";

/// Segments always kept at the end of a label: the file name and its folder.
const MIN_TAIL_SEGMENTS: usize = 2;

/// One label per connection, in the same order.
///
/// Labels never repeat. When two paths shorten to the same text, the part
/// that tells them apart was left out, so both are shown in full instead.
pub(super) fn recent_connection_labels(connections: &[RecentConnection]) -> Vec<String> {
    let shortened: Vec<String> = connections
        .iter()
        .map(|connection| shorten_path(&connection.url, MAX_LABEL_CHARS))
        .collect();

    shortened
        .iter()
        .zip(connections)
        .map(|(label, connection)| {
            let repeated = shortened.iter().filter(|other| *other == label).count() > 1;
            if repeated {
                connection.url.clone()
            } else {
                label.clone()
            }
        })
        .collect()
}

/// A path split into segments, together with the separator that joins them.
struct SplitPath<'a> {
    segments: Vec<&'a str>,
    separator: &'static str,
}

impl<'a> SplitPath<'a> {
    fn new(path: &'a str) -> Self {
        let separator = path_separator(path);
        Self {
            segments: path.split(separator).collect(),
            separator,
        }
    }

    /// The first `head` segments, an ellipsis, and the last `tail` segments.
    fn label(
        &self,
        head: usize,
        tail: usize,
    ) -> String {
        let mut parts: Vec<&str> = Vec::with_capacity(head + tail + 1);
        parts.extend_from_slice(&self.segments[..head]);
        parts.push(ELLIPSIS);
        parts.extend_from_slice(&self.segments[self.segments.len() - tail..]);
        parts.join(self.separator)
    }

    /// Whether that label leaves at least one segment out and is no longer
    /// than `max_chars`.
    fn fits(
        &self,
        head: usize,
        tail: usize,
        max_chars: usize,
    ) -> bool {
        head + tail < self.segments.len() && char_len(&self.label(head, tail)) <= max_chars
    }

    /// `head` extended by one named segment. The empty segments that stand
    /// for a leading separator are taken together with the name after them.
    fn next_head(
        &self,
        head: usize,
    ) -> usize {
        let mut next = head + 1;
        while next < self.segments.len() && self.segments[next - 1].is_empty() {
            next += 1;
        }
        next
    }
}

/// Shortens `path` to at most `max_chars` characters where possible.
///
/// The file name and its folder are always kept, even when they alone are
/// longer than `max_chars`. Further segments are then added alternately at
/// the end and at the start for as long as the label fits.
fn shorten_path(
    path: &str,
    max_chars: usize,
) -> String {
    if char_len(path) <= max_chars {
        return path.to_string();
    }

    let split = SplitPath::new(path);
    if split.segments.len() <= MIN_TAIL_SEGMENTS {
        return path.to_string();
    }

    let mut head = 0;
    let mut tail = MIN_TAIL_SEGMENTS;
    loop {
        let mut grew = false;

        if split.fits(head, tail + 1, max_chars) {
            tail += 1;
            grew = true;
        }

        let next_head = split.next_head(head);
        if split.fits(next_head, tail, max_chars) {
            head = next_head;
            grew = true;
        }

        if !grew {
            break;
        }
    }

    let label = split.label(head, tail);
    if char_len(&label) < char_len(path) {
        label
    } else {
        path.to_string()
    }
}

/// The separator `path` is written with. Stored paths are absolute, so a
/// Windows path contains only backslashes and every other path contains a
/// forward slash.
fn path_separator(path: &str) -> &'static str {
    if path.contains('\\') && !path.contains('/') {
        "\\"
    } else {
        "/"
    }
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{recent_connection_labels, shorten_path};
    use crate::config::{DatabaseBackend, RecentConnection};

    const LONG_PATH: &str = "/alpha/bravo/charlie/delta/echo/taxes.db";

    fn connection(url: &str) -> RecentConnection {
        RecentConnection {
            backend: DatabaseBackend::Sqlite,
            url: url.to_string(),
        }
    }

    #[test]
    fn path_within_the_limit_is_unchanged() {
        assert_eq!(shorten_path(LONG_PATH, 40), LONG_PATH);
    }

    #[test]
    fn long_path_keeps_the_end_first() {
        assert_eq!(shorten_path(LONG_PATH, 24), "…/delta/echo/taxes.db");
    }

    #[test]
    fn long_path_keeps_the_start_when_there_is_room() {
        assert_eq!(shorten_path(LONG_PATH, 30), "/alpha/…/delta/echo/taxes.db");
    }

    #[test]
    fn windows_path_is_shortened_with_backslashes() {
        let path = r"C:\Users\someone\Documents\Taxes\2025\taxes.db";

        assert_eq!(shorten_path(path, 30), r"C:\Users\…\Taxes\2025\taxes.db");
    }

    #[test]
    fn file_name_and_folder_are_kept_beyond_the_limit() {
        let path = "/a/b/a-very-long-folder-name/a-very-long-file-name.db";

        assert_eq!(
            shorten_path(path, 20),
            "…/a-very-long-folder-name/a-very-long-file-name.db"
        );
    }

    #[test]
    fn path_is_unchanged_when_shortening_would_not_help() {
        let path = "/dir/a-long-file-name.db";

        assert_eq!(shorten_path(path, 10), path);
    }

    #[test]
    fn default_limit_shortens_a_typical_long_path() {
        let path =
            "/Users/jonathan/Library/Mobile Documents/com~apple~CloudDocs/Taxes/2025/taxes.db";
        let labels = recent_connection_labels(&[connection(path)]);

        assert_eq!(
            labels,
            vec!["/Users/…/com~apple~CloudDocs/Taxes/2025/taxes.db".to_string()]
        );
    }

    #[test]
    fn same_file_name_in_different_folders_gets_different_labels() {
        let labels = recent_connection_labels(&[
            connection("/data/2025/taxes.db"),
            connection("/data/2024/taxes.db"),
        ]);

        assert_eq!(
            labels,
            vec![
                "/data/2025/taxes.db".to_string(),
                "/data/2024/taxes.db".to_string()
            ]
        );
    }

    #[test]
    fn labels_that_would_repeat_show_the_full_path() {
        let first = "/Volumes/Archive/clients/one/records/Taxes/2025/taxes.db";
        let second = "/Volumes/Archive/clients/two/records/Taxes/2025/taxes.db";
        let labels = recent_connection_labels(&[
            connection(first),
            connection("/data/taxes.db"),
            connection(second),
        ]);

        assert_eq!(
            labels,
            vec![
                first.to_string(),
                "/data/taxes.db".to_string(),
                second.to_string()
            ]
        );
    }
}
