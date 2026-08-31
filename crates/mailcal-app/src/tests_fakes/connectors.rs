//! On-demand mailbox-connector test fixtures: [`FakeConnector`] and [`FlakyConnector`].
//! Split out of `tests_fakes.rs` to keep each file under the size limit; a submodule of the
//! shared `fakes` module, handing back the folder-bound [`FakeProvider`] it defines.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use engine_core::mail::Message;
use mailcal_account::SyncDepth;

use super::FakeProvider;
use crate::{MailboxConnector, Surface};

/// A connector's shared, test-mutable folder contents: `(folder key, messages)` pairs.
type SharedFolders = Arc<Mutex<Vec<(String, Vec<Message>)>>>;

/// A fake on-demand connector: hands back a folder-bound [`FakeProvider`] preloaded with
/// the messages configured for the requested folder key, or `None` for an unknown folder.
/// The folder contents are shared and mutable ([`folders`](Self::folders)), so a test can
/// change what the "server" holds between connects; e.g. a folder whose messages were
/// renumbered under new keys after a `UIDVALIDITY` change.
pub(crate) struct FakeConnector {
    folders: SharedFolders,
}

impl FakeConnector {
    /// Builds a connector serving each `(folder key, messages)` pair on demand.
    pub(crate) fn new(folders: Vec<(String, Vec<Message>)>) -> Self {
        Self {
            folders: Arc::new(Mutex::new(folders)),
        }
    }

    /// A shared handle on the folder contents, so a test can replace a folder's messages
    /// between connects.
    pub(crate) fn folders(&self) -> SharedFolders {
        Arc::clone(&self.folders)
    }
}

#[async_trait::async_trait]
impl MailboxConnector<FakeProvider> for FakeConnector {
    async fn connect_folder(
        &self,
        _account: &AccountId,
        mailbox_key: &str,
        _depth: SyncDepth,
    ) -> Option<FakeProvider> {
        self.folders
            .lock()
            .unwrap()
            .iter()
            .find(|(key, _)| key == mailbox_key)
            .map(|(key, messages)| {
                let provider = FakeProvider::folder(key, messages.clone());
                // A folder scope that synced before re-syncs with a cursor, where the fake
                // serves its late-delivery queue: so hand the current contents there too,
                // and a *re*-sync (e.g. the UIDVALIDITY-conflict recovery) actually delivers
                // what the "server" now holds instead of an empty delta.
                provider
                    .late_delivery()
                    .lock()
                    .unwrap()
                    .extend(messages.iter().cloned());
                provider
            })
    }
}

/// A connector that fails (returns `None`) for the first `fail_times` connect attempts of
/// its folder, then serves it; models a transient connect failure (a network blip) so a
/// test can prove the folder still syncs on a later open instead of being blocked for the
/// session. Records every attempt so the test can assert the connect was retried.
pub(crate) struct FlakyConnector {
    folder_key: String,
    messages: Vec<Message>,
    remaining_failures: Mutex<u32>,
    attempts: Arc<Mutex<u32>>,
}

impl FlakyConnector {
    pub(crate) fn new(folder_key: &str, messages: Vec<Message>, fail_times: u32) -> Self {
        Self {
            folder_key: folder_key.to_owned(),
            messages,
            remaining_failures: Mutex::new(fail_times),
            attempts: Arc::new(Mutex::new(0)),
        }
    }

    /// A shared handle to the number of connect attempts made.
    pub(crate) fn attempts(&self) -> Arc<Mutex<u32>> {
        Arc::clone(&self.attempts)
    }
}

#[async_trait::async_trait]
impl MailboxConnector<FakeProvider> for FlakyConnector {
    async fn connect_folder(
        &self,
        _account: &AccountId,
        mailbox_key: &str,
        _depth: SyncDepth,
    ) -> Option<FakeProvider> {
        *self.attempts.lock().unwrap() += 1;
        if mailbox_key != self.folder_key {
            return None;
        }
        {
            let mut remaining = self.remaining_failures.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
                return None;
            }
        }
        Some(FakeProvider::folder(mailbox_key, self.messages.clone()))
    }
}

/// A connector that records how many [`Surface::MailboxList`] signals had been published by
/// the time it was asked to connect: the ordering test for "the folder is on screen before
/// the download starts". Serves one folder, like [`FakeConnector`].
pub(crate) struct ObservingConnector {
    key: String,
    messages: Vec<Message>,
    surfaces: Arc<Mutex<Vec<Surface>>>,
    published_before_connect: Arc<Mutex<Option<usize>>>,
}

impl ObservingConnector {
    /// Serves `messages` for `key`, watching `surfaces` for what the host had been told.
    pub(crate) fn new(
        key: &str,
        messages: Vec<Message>,
        surfaces: &Arc<Mutex<Vec<Surface>>>,
    ) -> Self {
        Self {
            key: key.to_owned(),
            messages,
            surfaces: Arc::clone(surfaces),
            published_before_connect: Arc::new(Mutex::new(None)),
        }
    }

    /// A shared handle on how many mailbox-list snapshots had been published when the connect
    /// began; `None` if it never was. Taken before the connector is handed to the app, which
    /// owns it from then on.
    pub(crate) fn published_before_connect(&self) -> Arc<Mutex<Option<usize>>> {
        Arc::clone(&self.published_before_connect)
    }
}

#[async_trait::async_trait]
impl MailboxConnector<FakeProvider> for ObservingConnector {
    async fn connect_folder(
        &self,
        _account: &AccountId,
        mailbox_key: &str,
        _depth: SyncDepth,
    ) -> Option<FakeProvider> {
        let published = self
            .surfaces
            .lock()
            .unwrap()
            .iter()
            .filter(|surface| **surface == Surface::MailboxList)
            .count();
        *self.published_before_connect.lock().unwrap() = Some(published);
        (mailbox_key == self.key).then(|| FakeProvider::folder(&self.key, self.messages.clone()))
    }
}
