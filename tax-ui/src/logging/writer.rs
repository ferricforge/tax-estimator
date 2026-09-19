//! Late-bound file output for log events.
//!
//! The file layer is installed at startup with a [`FileSlot`] writer. While
//! the slot is empty, formatted events are discarded; once a file is set,
//! events are appended to it.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};
use thiserror::Error;
use tracing_subscriber::fmt::MakeWriter;

/// Error returned when a log file cannot be opened.
#[derive(Debug, Error)]
pub(super) enum OpenLogFileError {
    /// A directory above the log file could not be created.
    #[error("cannot create log directory '{}'", .path.display())]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// The log file could not be created or opened for appending.
    #[error("cannot open log file '{}'", .path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Opens `path` for appending, creating the file and any missing parent
/// directories. A relative path is resolved against the current working
/// directory.
pub(super) fn open_log_file(path: &Path) -> Result<File, OpenLogFileError> {
    create_parent_directories(path)?;

    File::options()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| OpenLogFileError::Open {
            path: path.to_path_buf(),
            source,
        })
}

/// Creates the directories above `path` that do not exist yet. A bare file
/// name has no directories to create.
fn create_parent_directories(path: &Path) -> Result<(), OpenLogFileError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(parent).map_err(|source| OpenLogFileError::CreateDirectory {
        path: parent.to_path_buf(),
        source,
    })
}

/// A `MakeWriter` that can be pointed at a file after initialization.
/// While no file is set, all writes are discarded.
#[derive(Clone, Default)]
pub(super) struct FileSlot(Arc<Mutex<Option<File>>>);

impl FileSlot {
    /// Starts writing to `file`, closing any file that was set before.
    pub(super) fn set_file(
        &self,
        file: File,
    ) {
        *self.lock() = Some(file);
    }

    /// Closes the current file, if any. Later writes are discarded.
    pub(super) fn clear(&self) {
        *self.lock() = None;
    }

    /// Locks the slot, recovering the contents if a previous holder panicked.
    ///
    /// The slot holds only an optional file handle, which stays valid after a
    /// panic, so recovering is safe and keeps logging working.
    fn lock(&self) -> MutexGuard<'_, Option<File>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Writer returned by [`FileSlot`] for each event. Holds the slot lock for
/// the duration of the write.
pub(super) struct SlotWriter<'a>(MutexGuard<'a, Option<File>>);

impl Write for SlotWriter<'_> {
    fn write(
        &mut self,
        buf: &[u8],
    ) -> io::Result<usize> {
        match &mut *self.0 {
            Some(file) => file.write(buf),
            None => Ok(buf.len()), // discard silently when no file is set
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut *self.0 {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

impl<'a> MakeWriter<'a> for FileSlot {
    type Writer = SlotWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        SlotWriter(self.lock())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::thread;

    use crate::logging::test_support::{read_log, temp_dir};

    // --- Test helpers ---

    /// Panics while holding the slot lock, which poisons the mutex.
    fn poison(slot: &FileSlot) {
        let _guard = slot.0.lock().unwrap();
        panic!("intentional panic while holding the slot lock");
    }

    // --- FileSlot ---

    #[test]
    fn file_slot_discards_writes_when_no_file_is_set() {
        let slot = FileSlot::default();
        let mut writer = slot.make_writer();
        let content = b"discarded";

        assert_eq!(writer.write(content).unwrap(), content.len());
        writer.flush().expect("flush without a file must succeed");
    }

    #[test]
    fn file_slot_writes_only_while_a_file_is_set() {
        let dir = temp_dir();
        let path = dir.path().join("slot.log");
        let slot = FileSlot::default();
        let kept = "kept\n";

        slot.set_file(File::create(&path).unwrap());
        slot.make_writer().write_all(kept.as_bytes()).unwrap();

        // Clearing the slot closes the file; later writes are discarded.
        slot.clear();
        slot.make_writer().write_all(b"discarded\n").unwrap();

        assert_eq!(read_log(&path), kept);
    }

    #[test]
    fn file_slot_set_file_replaces_the_previous_file() {
        let dir = temp_dir();
        let first_path = dir.path().join("first.log");
        let second_path = dir.path().join("second.log");
        let slot = FileSlot::default();
        let first = "first\n";
        let second = "second\n";

        slot.set_file(File::create(&first_path).unwrap());
        slot.make_writer().write_all(first.as_bytes()).unwrap();

        slot.set_file(File::create(&second_path).unwrap());
        slot.make_writer().write_all(second.as_bytes()).unwrap();
        slot.clear();

        let actual = vec![read_log(&first_path), read_log(&second_path)];
        assert_eq!(actual, vec![first, second]);
    }

    #[test]
    fn file_slot_keeps_writing_after_the_lock_is_poisoned() {
        let dir = temp_dir();
        let path = dir.path().join("poisoned.log");
        let slot = FileSlot::default();
        let content = "after panic\n";
        slot.set_file(File::create(&path).unwrap());

        let poisoner = slot.clone();
        let handle = thread::spawn(move || poison(&poisoner));
        handle.join().expect_err("the helper thread must panic");

        // Precondition only. `assert_eq!(value, true)` is rejected by the
        // clippy `bool_assert_comparison` lint, and a diff of two booleans
        // adds no information.
        assert!(
            slot.0.is_poisoned(),
            "the slot lock must be poisoned before recovery is tested"
        );

        slot.make_writer().write_all(content.as_bytes()).unwrap();
        slot.clear();

        assert_eq!(read_log(&path), content);
    }

    // --- open_log_file ---

    #[test]
    fn open_log_file_creates_a_missing_file() {
        let dir = temp_dir();
        let path = dir.path().join("new.log");
        let content = "created\n";

        let mut file = open_log_file(&path).expect("log file must open");
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        assert_eq!(read_log(&path), content);
    }

    #[test]
    fn open_log_file_creates_missing_parent_directories() {
        let dir = temp_dir();
        let path = dir.path().join("logs").join("nested").join("app.log");
        let content = "created\n";

        let mut file = open_log_file(&path).expect("log file must open");
        file.write_all(content.as_bytes()).unwrap();
        drop(file);

        assert_eq!(read_log(&path), content);
    }

    #[test]
    fn open_log_file_appends_to_existing_content() {
        let dir = temp_dir();
        let path = dir.path().join("existing.log");
        let existing = "existing\n";
        let appended = "appended\n";
        fs::write(&path, existing).unwrap();

        let mut file = open_log_file(&path).expect("log file must open");
        file.write_all(appended.as_bytes()).unwrap();
        drop(file);

        assert_eq!(read_log(&path), format!("{existing}{appended}"));
    }

    #[test]
    fn open_log_file_reports_the_directory_when_a_parent_is_a_file() {
        let dir = temp_dir();
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, "").unwrap();
        let path = blocker.join("app.log");

        match open_log_file(&path).unwrap_err() {
            OpenLogFileError::CreateDirectory { path: failed, .. } => {
                assert_eq!(failed, blocker);
            }
            other => panic!("expected OpenLogFileError::CreateDirectory, got {other:?}"),
        }
    }

    #[test]
    fn open_log_file_reports_path_and_cause_when_the_path_is_a_directory() {
        let dir = temp_dir();
        let path = dir.path().to_path_buf();

        let error = open_log_file(&path).unwrap_err();
        match &error {
            OpenLogFileError::Open { path: failed, .. } => assert_eq!(failed, &path),
            other => panic!("expected OpenLogFileError::Open, got {other:?}"),
        }

        let chain: Vec<String> = anyhow::Error::from(error)
            .chain()
            .map(ToString::to_string)
            .collect();

        assert_eq!(chain.len(), 2);
        assert_eq!(
            chain[0],
            format!("cannot open log file '{}'", path.display())
        );
    }
}
