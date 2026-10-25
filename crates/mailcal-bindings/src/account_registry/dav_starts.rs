//! Each standards account's learned DAV principals: where its calendar and contacts connects
//! start (`mailcal_account::DavStarts`, RFC 6764 §6).
//!
//! They live on the registry entry's config, so every dial snapshots them, and in a file of their
//! own beside the stored configs, so the next launch starts from them too. A dial that learns a
//! different principal stores it; a change to the account's servers forgets them.

use mailcal_account::{DavStarts, DavStartsFile};

use super::AccountRegistry;
use crate::ConnectedAccount;

impl AccountRegistry {
    /// Keeps learned principals in `file` from now on. Called once, by the boot, before any
    /// account is registered.
    pub(crate) fn keep_dav_starts_in(&self, file: DavStartsFile) {
        *self.dav_starts.lock().expect("dav-starts mutex poisoned") = Some(file);
    }

    /// `entry` with the principals stored for `id` attached, when it is a standards account.
    pub(super) fn with_stored_dav_starts(
        &self,
        id: &str,
        mut entry: ConnectedAccount,
    ) -> ConnectedAccount {
        if let ConnectedAccount::Imap { config, .. } = &mut entry
            && let Some(file) = self
                .dav_starts
                .lock()
                .expect("dav-starts mutex poisoned")
                .as_ref()
        {
            config.dav_starts = file.load().remove(id).unwrap_or_default();
        }
        entry
    }

    /// Stores the principals a dial of `id` learned, where they differ from what it held.
    pub(crate) fn learn_dav_starts(&self, id: &str, learned: &DavStarts) {
        if learned.is_empty() {
            return;
        }
        let updated = {
            let mut entries = self
                .entries
                .lock()
                .expect("account registry mutex poisoned");
            let Some(ConnectedAccount::Imap { config, .. }) = entries.get_mut(id) else {
                return;
            };
            let updated = config.dav_starts.updated_with(learned);
            if updated == config.dav_starts {
                return;
            }
            config.dav_starts = updated.clone();
            updated
        };
        self.store_dav_starts(id, &updated);
    }

    /// Forgets `id`'s principals: its servers changed, or it was removed. Its next connect starts
    /// from the configured servers.
    pub(crate) fn forget_dav_starts(&self, id: &str) {
        if let Some(ConnectedAccount::Imap { config, .. }) = self
            .entries
            .lock()
            .expect("account registry mutex poisoned")
            .get_mut(id)
        {
            config.dav_starts = DavStarts::default();
        }
        self.store_dav_starts(id, &DavStarts::default());
    }

    fn store_dav_starts(&self, id: &str, starts: &DavStarts) {
        // Held across the write: the file is read, changed and written whole, and dials run
        // concurrently, so two stores interleaved would lose one account's principals.
        let file = self.dav_starts.lock().expect("dav-starts mutex poisoned");
        if let Some(file) = file.as_ref()
            && let Err(error) = file.store(id, starts)
        {
            // The cache only saves the next connect a step: it starts from the configured server
            // instead.
            log::warn!("dav: the learned servers could not be saved: {error}");
        }
    }
}

#[cfg(test)]
#[path = "dav_starts_tests.rs"]
mod tests;
