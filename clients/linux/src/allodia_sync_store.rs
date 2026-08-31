//! Where this device remembers what it has synced with the Allodia account service.
//!
//! Its Apple, Android and Windows twins keep the same blob in each platform's own ordinary
//! preferences. A plain file and not the Secret Service: nothing in the blob is secret (a record
//! id, a version, a fingerprint, a flag), and a keyring prompt in front of a pass nobody started
//! would be a prompt nobody is there to answer.
//!
//! It sits beside the engine store rather than in [`crate::preferences`], so a dev-account launch's
//! bookkeeping is isolated with the accounts it describes; that store is already per-launch, and
//! `host.json` is not.
//!
//! Unlike the host preferences next door, a failed write is **reported** rather than swallowed: by
//! the time the core calls this it has already written to the service, and a note that never landed
//! is a record this device will offer itself back at the next pass.

use std::{
    fs,
    path::{Path, PathBuf},
};

use mailcal_bindings::{SyncStateError, SyncStateStore};

/// The sync bookkeeping, in one file, written whole.
#[derive(Debug)]
pub(crate) struct FileSyncStateStore {
    path: PathBuf,
}

impl FileSyncStateStore {
    /// The store for the engine directory `data_dir` this launch opened.
    pub(crate) fn in_data_dir(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("allodia-sync.json"),
        }
    }
}

impl SyncStateStore for FileSyncStateStore {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        match fs::read_to_string(&self.path) {
            Ok(blob) => Ok(Some(blob)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            // Not "never synced": that would start a pass which re-adopts every record.
            Err(error) => Err(SyncStateError::Store(error.to_string())),
        }
    }

    fn save(&self, blob: String) -> Result<(), SyncStateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| SyncStateError::Store(error.to_string()))?;
        }
        // Through a temporary, like the host preferences: a blob half-written by a process that
        // died is worse than an old one, because the entries it lost are records this device would
        // upload a second time.
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, blob).map_err(|error| SyncStateError::Store(error.to_string()))?;
        fs::rename(&temporary, &self.path).map_err(|error| SyncStateError::Store(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use mailcal_bindings::SyncStateStore as _;

    use super::FileSyncStateStore;

    fn scratch() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mailcal-linux-sync-{nonce}"))
    }

    #[test]
    fn a_device_that_has_never_synced_reads_back_nothing() {
        let store = FileSyncStateStore::in_data_dir(&scratch());
        assert_eq!(store.load().expect("an absent file is not an error"), None);
    }

    /// The write creates the directory it needs. The engine store's own directory exists by the
    /// time a pass runs, but a store handed a path before that must not fail on it.
    #[test]
    fn a_blob_survives_the_round_trip_and_makes_its_own_directory() {
        let dir = scratch();
        let store = FileSyncStateStore::in_data_dir(&dir);
        store.save("{\"a\":1}".to_owned()).expect("stored");
        assert_eq!(
            store.load().expect("readable").as_deref(),
            Some("{\"a\":1}")
        );
        store.save("{\"a\":2}".to_owned()).expect("replaced whole");
        assert_eq!(
            store.load().expect("readable").as_deref(),
            Some("{\"a\":2}")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
