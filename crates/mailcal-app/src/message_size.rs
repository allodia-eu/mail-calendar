//! The per-account **message-size** setting: how big a message this account keeps offline.
//!
//! A second `impl App` block, kept out of `sync_settings.rs`, which owns how mail *arrives*
//! (push vs poll, the watched folders, the depth); both so each file stays under the 500-line
//! cap and because this one is about what is kept rather than what is fetched.

use engine_api::{AccountId, Provider};
use mailcal_account::{AccountSyncSettings, MessageSizeLimit};

use crate::{App, form_factor::FormFactor};

impl<P: Provider> App<P> {
    /// Sets an account's message-size cap (its explicit per-account override) as a megabyte
    /// count (`0` = no limit), leaving its strategy / folders / interval / depth untouched.
    /// Persists and signals [`Surface::Settings`](crate::Surface::Settings). This only changes the
    /// stored setting; user-facing changes go through
    /// [`update_account_message_size_limit`](Self::update_account_message_size_limit), which
    /// acts on the mail already cached.
    pub async fn set_account_message_size_limit(&self, id: &str, megabytes: u16) {
        let Some(eff) = self.effective_for(id).await else {
            return;
        };
        self.store_entry(
            id,
            AccountSyncSettings {
                strategy: eff.strategy,
                push_folders: eff.push_folders,
                poll_interval_mins: eff.poll_interval_mins,
                sync_depth: self.stored_depth_override(id),
                message_size_limit: Some(MessageSizeLimit::from(megabytes)),
            },
        );
    }

    /// Applies a user-facing message-size change and acts on what is already cached.
    ///
    /// The two directions are not symmetric. **Raising** it only has to stop skipping: the
    /// messages it now admits never had a body cached, so they are already on the warm work
    /// list and a pass picks them up. **Lowering** it has to undo a warm, and that runs
    /// before any network call; "keep less of my mail on this device" is a decision about
    /// this device, and a user freeing space on a plane must not have to wait for a server.
    pub async fn update_account_message_size_limit(&self, id: &str, megabytes: u16) {
        let Ok(account_id) = AccountId::try_from(id) else {
            return;
        };
        if self.account_handle(&account_id).await.is_none() {
            return;
        }
        let before = self.effective_message_size_limit(id);
        self.set_account_message_size_limit(id, megabytes).await;
        let after = self.effective_message_size_limit(id);
        if before == after {
            return;
        }
        if is_smaller(after, before) {
            self.drop_bodies_over(&account_id, after).await;
            self.reclaim_freed_space("message-size").await;
        } else {
            self.prefetch_account_bodies(&account_id).await;
        }
    }

    /// Forgets the cached bodies this account may no longer keep, with no provider round trip.
    /// The mail itself stays: only the offline copy of the heaviest messages goes, so the list,
    /// the threads and body search are unchanged.
    async fn drop_bodies_over(&self, id: &AccountId, limit: Option<u64>) {
        let Some(octets) = limit else { return };
        let acct = self.account_ordinal(id).await;
        match self.engine.drop_message_sources_over(id, octets).await {
            Ok(report) => log::info!(
                "message-size[a{acct}]: dropped {} cached body/bodies holding {} KiB",
                report.sources_removed,
                report.octets_freed / 1024,
            ),
            Err(err) => log::warn!("message-size[a{acct}]: dropping cached bodies failed: {err}"),
        }
    }

    /// The account's explicit message-size override, if any.
    pub(crate) fn stored_size_override(&self, id: &str) -> Option<MessageSizeLimit> {
        self.sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .size_override(id)
    }

    /// The message-size cap **in effect** for an account, in octets; `None` warms every size.
    ///
    /// Its override, else the default for this kind of device, which is why the resolution
    /// lives here and not in `mailcal-account`, where the form factor is unknown.
    #[must_use]
    pub fn effective_message_size_limit(&self, id: &str) -> Option<u64> {
        self.stored_size_override(id).map_or_else(
            || FormFactor::current().default_prefetch_size_limit(),
            MessageSizeLimit::octets,
        )
    }
}

/// The product default cap as a megabyte count, for the picker to show a preselected option;
/// `0` where this kind of device warms every size. Rounded down, so an option a client offers is
/// never reported as one it does not.
pub(crate) fn default_size_limit_mb() -> u16 {
    FormFactor::current()
        .default_prefetch_size_limit()
        .map_or(0, |octets| {
            u16::try_from(octets / (1024 * 1024)).unwrap_or(u16::MAX)
        })
}

/// Whether `next` keeps less than `previous`, treating "no limit" as larger than any cap.
pub(crate) fn is_smaller(next: Option<u64>, previous: Option<u64>) -> bool {
    match (next, previous) {
        (Some(next), Some(previous)) => next < previous,
        (Some(_), None) => true,
        (None, _) => false,
    }
}
