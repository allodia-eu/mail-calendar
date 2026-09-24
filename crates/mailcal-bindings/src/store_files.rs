//! The files under the data directory that make up the local mail store.

use std::path::Path;

/// The database's file name inside the data directory.
pub(crate) const STORE_FILE: &str = "mailcal.sqlite";

/// The paths under `data_dir` that hold the local mail store: the database, its WAL and
/// shared-memory files, and the blob directory the engine keeps beside it (message sources and
/// contact photos). Everything here is rebuilt by a sync from the server, so a client keeps it out
/// of device backups. Preferences, signatures and the log sit beside it and are not listed.
///
/// A path may not exist yet: the WAL and shared-memory files come and go with the connection.
#[uniffi::export]
#[must_use]
pub fn mail_store_paths(data_dir: String) -> Vec<String> {
    let db = Path::new(&data_dir).join(STORE_FILE);
    ["", "-wal", "-shm", ".blobs"]
        .iter()
        .map(|suffix| format!("{}{suffix}", db.display()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_file_the_engine_creates_is_a_listed_store_path() {
        let dir = std::env::temp_dir().join(format!("mailcal-store-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let engine = engine_api::Engine::open(dir.join(STORE_FILE)).expect("open store");

        let listed: BTreeSet<String> = mail_store_paths(dir.display().to_string())
            .into_iter()
            .collect();
        let created: BTreeSet<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path().display().to_string())
            .collect();
        assert!(created.contains(&dir.join(STORE_FILE).display().to_string()));
        assert!(
            created.contains(&dir.join("mailcal.sqlite.blobs").display().to_string()),
            "the engine keeps message sources in a directory beside the database: {created:?}",
        );
        assert!(
            created.is_subset(&listed),
            "{created:?} not all in {listed:?}"
        );
        assert!(
            !listed.iter().any(|path| path.ends_with("preferences.toml")),
            "preferences stay in the backup",
        );

        drop(engine);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
