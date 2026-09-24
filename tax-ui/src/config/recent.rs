//! The `[recent]` section of the configuration file.

use serde::{Deserialize, Serialize};

use super::{DatabaseBackend, database::SQLITE_IN_MEMORY_URL};

/// Number of recent connections kept when the configuration does not say.
const DEFAULT_LIMIT: usize = 10;

// ---------------------------------------------------------------------------
// RecentConnection
// ---------------------------------------------------------------------------

/// One entry under `[[recent.connections]]`: a database that was open earlier
/// and can be opened again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentConnection {
    /// Backend that opens [`url`](Self::url).
    #[serde(default)]
    pub backend: DatabaseBackend,

    /// Backend-specific location. For SQLite: an absolute file path.
    pub url: String,
}

impl RecentConnection {
    /// Builds the entry for `url` on `backend`, or `None` when that database
    /// cannot be opened again later.
    ///
    /// For SQLite, `":memory:"` cannot be reopened, and a relative path is
    /// made absolute so the entry does not depend on the working directory
    /// and the same file is always written the same way.
    pub fn for_database(
        backend: DatabaseBackend,
        url: &str,
    ) -> Option<Self> {
        let url = match backend {
            DatabaseBackend::Sqlite => sqlite_reopenable_url(url)?,
        };
        Some(Self { backend, url })
    }

    /// Whether the database this entry names still exists.
    pub fn exists(&self) -> bool {
        self.backend.location_exists(&self.url)
    }
}

/// The absolute form of a SQLite file location, or `None` for a location
/// that cannot be reopened.
fn sqlite_reopenable_url(url: &str) -> Option<String> {
    if url.is_empty() || url == SQLITE_IN_MEMORY_URL {
        return None;
    }

    let absolute = std::path::absolute(url)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| url.to_string());
    Some(absolute)
}

// ---------------------------------------------------------------------------
// RecentConfig
// ---------------------------------------------------------------------------

/// Recently used connections, stored in the `[recent]` section of the
/// configuration.
///
/// The application maintains the list; `limit` is the only value meant to be
/// edited by hand. The list never contains the connection that is currently
/// open, and never contains the same connection twice.
///
/// ```toml
/// [recent]
/// limit = 10
///
/// [[recent.connections]]
/// backend = "sqlite"
/// url = "/home/me/taxes/2024.db"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentConfig {
    /// Most entries kept in [`connections`](Self::connections). `0` keeps
    /// none.
    pub limit: usize,

    /// Earlier connections, most recent first.
    pub connections: Vec<RecentConnection>,
}

impl Default for RecentConfig {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            connections: Vec::new(),
        }
    }
}

impl RecentConfig {
    /// Updates the list after the application switches connections.
    ///
    /// `previous` is the connection that was open before the switch; it moves
    /// to the front of the list. `current` is the connection that is open
    /// now; it is removed from the list. Pass `None` for a connection that
    /// cannot be reopened (see [`RecentConnection::for_database`]).
    pub fn record_switch(
        &mut self,
        previous: Option<RecentConnection>,
        current: Option<&RecentConnection>,
    ) {
        if let Some(previous) = previous {
            self.connections.retain(|entry| *entry != previous);
            self.connections.insert(0, previous);
        }

        if let Some(current) = current {
            self.connections.retain(|entry| entry != current);
        }

        self.connections.truncate(self.limit);
    }

    /// Removes entries whose database no longer exists and returns how many
    /// were removed.
    pub fn remove_missing(&mut self) -> usize {
        let before = self.connections.len();
        self.connections.retain(RecentConnection::exists);
        before - self.connections.len()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::{DatabaseBackend, RecentConfig, RecentConnection};

    fn entry(url: &str) -> RecentConnection {
        RecentConnection {
            backend: DatabaseBackend::Sqlite,
            url: url.to_string(),
        }
    }

    #[test]
    fn previous_connection_goes_to_the_front() {
        let mut recent = RecentConfig::default();

        recent.record_switch(Some(entry("a.db")), Some(&entry("b.db")));
        recent.record_switch(Some(entry("b.db")), Some(&entry("c.db")));

        assert_eq!(recent.connections, vec![entry("b.db"), entry("a.db")]);
    }

    #[test]
    fn listed_connection_moves_instead_of_repeating() {
        let mut recent = RecentConfig {
            connections: vec![entry("a.db"), entry("b.db"), entry("c.db")],
            ..RecentConfig::default()
        };

        recent.record_switch(Some(entry("c.db")), Some(&entry("d.db")));

        assert_eq!(
            recent.connections,
            vec![entry("c.db"), entry("a.db"), entry("b.db")]
        );
    }

    #[test]
    fn current_connection_is_removed_from_the_list() {
        let mut recent = RecentConfig {
            connections: vec![entry("a.db"), entry("b.db")],
            ..RecentConfig::default()
        };

        recent.record_switch(Some(entry("c.db")), Some(&entry("b.db")));

        assert_eq!(recent.connections, vec![entry("c.db"), entry("a.db")]);
    }

    #[test]
    fn reopening_the_current_connection_lists_nothing() {
        let mut recent = RecentConfig::default();

        recent.record_switch(Some(entry("a.db")), Some(&entry("a.db")));

        assert_eq!(recent.connections, Vec::new());
    }

    #[test]
    fn list_is_cut_to_the_limit() {
        let mut recent = RecentConfig {
            limit: 2,
            connections: vec![entry("a.db"), entry("b.db")],
        };

        recent.record_switch(Some(entry("c.db")), Some(&entry("d.db")));

        assert_eq!(recent.connections, vec![entry("c.db"), entry("a.db")]);
    }

    #[test]
    fn zero_limit_keeps_no_entries() {
        let mut recent = RecentConfig {
            limit: 0,
            connections: Vec::new(),
        };

        recent.record_switch(Some(entry("a.db")), Some(&entry("b.db")));

        assert_eq!(recent.connections, Vec::new());
    }

    #[test]
    fn sqlite_in_memory_database_is_not_listed() {
        let entry = RecentConnection::for_database(DatabaseBackend::Sqlite, ":memory:");

        assert_eq!(entry, None);
    }

    #[test]
    fn sqlite_relative_path_is_stored_as_absolute() {
        let entry = RecentConnection::for_database(DatabaseBackend::Sqlite, "taxes.db")
            .expect("a file database can be reopened");

        assert!(Path::new(&entry.url).is_absolute());
        assert!(entry.url.ends_with("taxes.db"));
    }

    #[test]
    fn sqlite_absolute_path_is_kept() {
        let first = RecentConnection::for_database(DatabaseBackend::Sqlite, "taxes.db")
            .expect("a file database can be reopened");
        let second = RecentConnection::for_database(DatabaseBackend::Sqlite, &first.url);

        assert_eq!(second, Some(first));
    }

    #[test]
    fn remove_missing_drops_only_absent_files() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let present = dir.path().join("present.db");
        std::fs::write(&present, b"").expect("file should be written");
        let present_url = present.to_string_lossy().into_owned();
        let absent_url = dir.path().join("absent.db").to_string_lossy().into_owned();
        let mut recent = RecentConfig {
            connections: vec![entry(&absent_url), entry(&present_url)],
            ..RecentConfig::default()
        };

        let removed = recent.remove_missing();

        assert_eq!(removed, 1);
        assert_eq!(recent.connections, vec![entry(&present_url)]);
    }
}
