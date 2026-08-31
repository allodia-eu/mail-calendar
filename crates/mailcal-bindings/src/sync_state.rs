// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! Where a device remembers what it knows about syncing its accounts.
//!
//! One blob rather than an entry per account, and it is written whole. What it holds is a *set* of
//! relationships (an id claimed here is an id not offered there) so a half-written map is worse
//! than an old one: the pass that reads it would upload duplicates for whichever accounts lost
//! their entry. Hosts get one read and one write, both atomic by construction.
//!
//! **Nothing secret is in here.** The passwords stay in [`crate::credential_store`]; this is
//! bookkeeping (a server id, a version, a fingerprint, a flag) and it goes wherever the host
//! keeps ordinary preferences rather than in the keystore.
//!
//! The port ships in every build, like the rest of the Allodia FFI surface; a build with no
//! Allodia registration simply never calls it.

use std::sync::Mutex;

/// Why a host could not read or write the sync bookkeeping.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SyncStateError {
    /// The platform store refused.
    #[error("the sync state could not be stored: {0}")]
    Store(String),
}

/// Where the host keeps this device's sync bookkeeping.
///
/// Ordinary preferences, not the keystore: nothing here is secret, and putting it behind a
/// keychain prompt would make a background pass wait on a person.
///
/// Implementations must be safe to call from any thread.
#[uniffi::export(callback_interface)]
pub trait SyncStateStore: Send + Sync {
    /// The blob last written, or `None` if this device has never synced.
    ///
    /// # Errors
    ///
    /// Throws [`SyncStateError`] when the platform store could not be read. Report it rather than
    /// answering `None`: "never synced" starts a first-run pass that would re-upload every
    /// account, and a store that is merely unreadable today must not look like that.
    fn load(&self) -> Result<Option<String>, SyncStateError>;

    /// Replaces the blob, whole.
    ///
    /// # Errors
    ///
    /// Throws [`SyncStateError`] when the platform store refused the write.
    fn save(&self, blob: String) -> Result<(), SyncStateError>;
}

/// The store for a build with nothing to remember: the in-memory demo and showcase apps.
///
/// It answers "never synced" and accepts every write without keeping it, which is truthful for a
/// core whose accounts are bundled fixtures; unlike the credential store next door, nothing here
/// is lost by not persisting, because a fixture account is rebuilt at every launch.
#[derive(Debug, Default)]
pub struct NoSyncState;

impl SyncStateStore for NoSyncState {
    fn load(&self) -> Result<Option<String>, SyncStateError> {
        Ok(None)
    }

    fn save(&self, _blob: String) -> Result<(), SyncStateError> {
        Ok(())
    }
}

/// The bookkeeping itself, held in memory and written through to the host.
///
/// A cache in front of the port, because a reconcile pass reads it once per account and a host's
/// preferences read may be a file. The write is still whole and still immediate: a pass that
/// updated memory and deferred the store would, if the app died between them, upload duplicates at
/// the next launch for every account whose id it had forgotten.
#[derive(Debug)]
pub struct SyncBookkeeping {
    store: Box<dyn SyncStateStore>,
    blob: Mutex<Blob>,
}

impl std::fmt::Debug for dyn SyncStateStore + '_ {
    /// The host's own type is not ours to describe, and a port has no state worth printing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SyncStateStore")
    }
}

/// The blob's shape: account id to what this device knows about it.
pub type Entries = std::collections::BTreeMap<String, StoredSyncState>;

/// One account's entry, as it sits in the blob.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredSyncState {
    /// The service's id for this account.
    pub id: String,
    /// The version this device last read.
    pub version: u64,
    /// What the config looked like at that version.
    pub fingerprint: String,
}

/// Everything the blob holds.
///
/// **Excluding an account is not part of its entry**, and the difference is load-bearing: an entry
/// records a conversation this device has had with the service, and a person can decide an account
/// should not travel before there has ever been one. Kept inside the entry, that decision would
/// have nowhere to live until the account had already been sent, which is the one moment it must
/// not be.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Blob {
    /// What this device knows about each account it has synced.
    #[serde(default)]
    accounts: Entries,
    /// Accounts the person told this device to keep to itself.
    #[serde(default)]
    excluded: std::collections::BTreeSet<String>,
    /// The idempotency key of a create that has been attempted and not yet confirmed.
    ///
    /// **One key per create, not one per account.** A key derived from the account is the same for
    /// its whole life, so after the record is deleted the next create replays the first one and is
    /// refused: for ever, because neither the key nor the answer ever changes. Each delete stacks
    /// another layer, so no fixed number of retries unwinds it either.
    ///
    /// Minted when an upload is first attempted, reused by every retry of *that* upload, and
    /// dropped the moment the service confirms it. An account with no entry here is one whose next
    /// create is genuinely new.
    #[serde(default)]
    pending_creates: std::collections::BTreeMap<String, String>,
}

impl SyncBookkeeping {
    /// Read what the host holds. An unreadable blob is reported; an *absent* one is a first run.
    ///
    /// A blob that will not parse is treated as absent and logged rather than refused. It is
    /// bookkeeping, not data: the worst a fresh start costs is one pass that adopts the records it
    /// finds, and refusing would leave sync permanently broken on a device with one bad byte.
    pub fn load(store: Box<dyn SyncStateStore>) -> Result<Self, SyncStateError> {
        let blob = match store.load()? {
            Some(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
                log::warn!(
                    "allodia: the sync bookkeeping could not be read ({error}); starting from \
                     nothing, which re-adopts the records the service already holds"
                );
                Blob::default()
            }),
            None => Blob::default(),
        };
        Ok(Self {
            store,
            blob: Mutex::new(blob),
        })
    }

    /// What this device knows about `account_id`.
    pub fn get(&self, account_id: &str) -> Option<StoredSyncState> {
        self.blob
            .lock()
            .expect("sync bookkeeping lock")
            .accounts
            .get(account_id)
            .cloned()
    }

    /// Every entry, for a caller building the reconciler's input.
    pub fn all(&self) -> Entries {
        self.blob
            .lock()
            .expect("sync bookkeeping lock")
            .accounts
            .clone()
    }

    /// The key of a create attempted for `account_id` and not yet confirmed.
    pub fn pending_create_key(&self, account_id: &str) -> Option<String> {
        self.blob
            .lock()
            .expect("sync bookkeeping lock")
            .pending_creates
            .get(account_id)
            .cloned()
    }

    /// Records the key of a create about to be attempted, or drops it once it is confirmed.
    ///
    /// # Errors
    ///
    /// As [`SyncBookkeeping::set`].
    pub fn set_pending_create_key(
        &self,
        account_id: &str,
        key: Option<&str>,
    ) -> Result<(), SyncStateError> {
        self.write(|blob| match key {
            Some(key) => {
                blob.pending_creates
                    .insert(account_id.to_owned(), key.to_owned());
            }
            None => {
                blob.pending_creates.remove(account_id);
            }
        })
    }

    /// Whether the person told this device to keep `account_id` to itself.
    pub fn is_excluded(&self, account_id: &str) -> bool {
        self.blob
            .lock()
            .expect("sync bookkeeping lock")
            .excluded
            .contains(account_id)
    }

    /// Records whether `account_id` travels, and writes it through.
    ///
    /// # Errors
    ///
    /// As [`SyncBookkeeping::set`].
    pub fn set_excluded(&self, account_id: &str, excluded: bool) -> Result<(), SyncStateError> {
        self.write(|blob| {
            if excluded {
                blob.excluded.insert(account_id.to_owned());
            } else {
                blob.excluded.remove(account_id);
            }
        })
    }

    /// Record what this device now knows, and write it through.
    ///
    /// # Errors
    ///
    /// Throws [`SyncStateError`] when the host refused the write. The in-memory copy is updated
    /// either way: a pass that already wrote to the service and then failed to note it here must
    /// not go on to write again in the same pass.
    pub fn set(&self, account_id: &str, state: StoredSyncState) -> Result<(), SyncStateError> {
        self.write(|blob| {
            blob.accounts.insert(account_id.to_owned(), state);
        })
    }

    /// Forget an account entirely; it was removed here, or its record was.
    ///
    /// # Errors
    ///
    /// As [`SyncBookkeeping::set`].
    pub fn forget(&self, account_id: &str) -> Result<(), SyncStateError> {
        self.write(|blob| {
            blob.accounts.remove(account_id);
            // The account is gone from this device, so the choice made about it is too. Keeping it
            // would silently exclude a re-added account of the same name months later.
            blob.excluded.remove(account_id);
            // And any create still in flight for it: the next one is a new account's, whatever
            // its address.
            blob.pending_creates.remove(account_id);
        })
    }

    /// Forget every conversation this device has had with the service, on sign-out.
    ///
    /// The entries and the in-flight creates describe **one Allodia account's** records, so they
    /// mean nothing to the next sign-in and are actively wrong if it is a different account: an id
    /// that matches no record claims nothing, and a device that claims nothing is offered back the
    /// mail accounts it already has.
    ///
    /// **The exclusions stay.** They are not a conversation with the service; they are what the
    /// person answered about *this device's* mail accounts, and dropping them would put an account
    /// somebody deliberately kept off back on the wire at the next pass. What is lost is only what
    /// the next pass re-derives: with no entry, each local account matches the record the service
    /// already holds and adopts it.
    ///
    /// # Errors
    ///
    /// As [`SyncBookkeeping::set`].
    pub fn forget_the_session(&self) -> Result<(), SyncStateError> {
        self.write(|blob| {
            blob.accounts.clear();
            blob.pending_creates.clear();
        })
    }

    /// Change the blob and write it through, under one lock and never across the host's write.
    ///
    /// The in-memory copy advances whatever the host does: a pass that has already written to the
    /// service and then failed to note it must not go on to write again in the same pass.
    fn write(&self, change: impl FnOnce(&mut Blob)) -> Result<(), SyncStateError> {
        let text = {
            let mut blob = self.blob.lock().expect("sync bookkeeping lock");
            change(&mut blob);
            serde_json::to_string(&*blob)
                .map_err(|error| SyncStateError::Store(error.to_string()))?
        };
        self.store.save(text)
    }
}

#[cfg(test)]
#[path = "sync_state_tests.rs"]
mod sync_state_tests;
