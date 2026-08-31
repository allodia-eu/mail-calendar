//! The per-account synchronisation-behaviour state machine: which accounts receive mail
//! as it arrives (IMAP `IDLE` push) versus check on a timer, and persistence.
//!
//! Like the display-timezone setting ([`crate::timezone`]), this is a host **app
//! preference**, not synced PIM data; it lives in the shared `preferences.toml` via
//! [`mailcal_account`]. The product-core owns the state machine; a host renders the
//! immutable [`SyncSettingsSnapshot`] and drives the mutators. The actual push/poll
//! *runtime* (the standing `IDLE` connections and poll timers) is the bindings layer's
//! job; it reads this snapshot to decide what to run; this module only owns the
//! **configuration** and its defaulting.

use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use engine_api::{AccountId, Mailbox, MailboxRole, Provider};
use mailcal_account::{
    AccountSyncSettings, EffectiveSync, MAX_PUSH_FOLDERS, MESSAGE_SIZE_LIMITS_MB, MessageSizeLimit,
    POLL_INTERVALS, SYNC_DEPTHS, SyncDepth, SyncStrategy, cap_push_folders, effective,
    load_preferences, save_preferences, snap_poll_interval,
};
use mailcal_viewmodel::{
    AccountSyncRow, SyncFolderRow, SyncSettingsSnapshot, SyncStrategyKind, folder_role,
};

use crate::{App, Surface, message_size::default_size_limit_mb, sync::sync_window};

/// The persisted per-account sync settings + where to write them. Holds the stored
/// (customised) per-account choices. An account absent from `stored` uses the [`effective`]
/// push/poll default, resolved against the live `IDLE` capability when a snapshot is built.
pub(crate) struct SyncSettingsState {
    stored: BTreeMap<String, AccountSyncSettings>,
    prefs_path: Option<PathBuf>,
}

impl SyncSettingsState {
    /// Loads the stored per-account settings from the preferences file (if any).
    pub(crate) fn new(prefs_path: Option<PathBuf>) -> Self {
        let prefs = prefs_path
            .as_ref()
            .map(load_preferences)
            .unwrap_or_default();
        Self {
            stored: prefs.accounts,
            prefs_path,
        }
    }

    /// The stored (customised) settings for an account, if it has any.
    fn get(&self, id: &str) -> Option<AccountSyncSettings> {
        self.stored.get(id).cloned()
    }

    /// The account's explicit sync-depth override, if any; read to preserve it when another
    /// field of its settings changes (the setters build a fresh [`AccountSyncSettings`]).
    fn depth_override(&self, id: &str) -> Option<SyncDepth> {
        self.stored.get(id).and_then(|entry| entry.sync_depth)
    }

    /// The sync depth **in effect** for an account: its override, else the product default.
    fn effective_depth(&self, id: &str) -> SyncDepth {
        self.depth_override(id).unwrap_or_default()
    }

    /// The account's explicit message-size override, if any; read to preserve it when another
    /// field of its settings changes, exactly as the depth override is.
    pub(crate) fn size_override(&self, id: &str) -> Option<MessageSizeLimit> {
        self.stored
            .get(id)
            .and_then(|entry| entry.message_size_limit)
    }

    /// Inserts/updates one account's settings and persists. Read-modify-write so the
    /// sibling display-zone / grouping / quote-style preferences in the same file are preserved.
    fn set_entry(&mut self, id: &str, entry: AccountSyncSettings) {
        self.stored.insert(id.to_owned(), entry);
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.accounts = self.stored.clone();
            let _ = save_preferences(path, &prefs);
        }
    }

    /// Removes one account's stored settings and persists, so a later re-add starts from the
    /// product defaults instead of resurrecting stale per-account choices.
    fn remove_entry(&mut self, id: &str) {
        self.stored.remove(id);
        if let Some(path) = &self.prefs_path {
            let mut prefs = load_preferences(path);
            prefs.accounts = self.stored.clone();
            let _ = save_preferences(path, &prefs);
        }
    }
}

/// The Inbox folder key of an account (resolved by role), as a one-element default push
/// set; what a newly-pushed account watches before the user picks folders.
fn inbox_default(mailboxes: &[Mailbox]) -> Vec<String> {
    mailboxes
        .iter()
        .find(|mailbox| mailbox.role == Some(MailboxRole::Inbox))
        .map(|mailbox| mailbox.id.key().as_str().to_owned())
        .into_iter()
        .collect()
}

/// Maps the persisted strategy to the host-facing kind.
fn kind(strategy: SyncStrategy) -> SyncStrategyKind {
    match strategy {
        SyncStrategy::Push => SyncStrategyKind::Push,
        SyncStrategy::Poll => SyncStrategyKind::Poll,
    }
}

/// Maps the host-facing kind back to the persisted strategy.
fn to_strategy(kind: SyncStrategyKind) -> SyncStrategy {
    match kind {
        SyncStrategyKind::Push => SyncStrategy::Push,
        SyncStrategyKind::Poll => SyncStrategy::Poll,
    }
}

impl<P: Provider> App<P> {
    /// The per-account synchronisation-behaviour snapshot a host renders (pulled after a
    /// [`Surface::Settings`] signal): one row per account with its effective strategy,
    /// poll interval, `IDLE` support, and the folder list with push-subscription state.
    /// The bindings' background runtime also reads this to decide what to watch/poll.
    #[must_use]
    pub async fn sync_settings(&self) -> SyncSettingsSnapshot {
        let accounts = self.account_handles().await;
        let mut rows = Vec::with_capacity(accounts.len());
        for account in &accounts {
            // IDLE is a server property, so any provider's capability answers for the
            // account; checking all is cheap and robust to which folder bound first.
            let idle_supported = account
                .providers
                .iter()
                .any(|p| p.connection_info().capabilities.idle());
            let mailboxes = self.engine.mailboxes(&account.id).await.unwrap_or_default();
            let default_push = inbox_default(&mailboxes);
            let (stored, sync_depth_months) = {
                let guard = self
                    .sync_settings
                    .lock()
                    .expect("sync-settings mutex poisoned");
                (
                    guard.get(account.id.as_str()),
                    u16::from(guard.effective_depth(account.id.as_str())),
                )
            };
            let eff = effective(stored.as_ref(), idle_supported, &default_push);
            let subscribed: HashSet<&str> = eff.push_folders.iter().map(String::as_str).collect();
            let is_push = matches!(eff.strategy, SyncStrategy::Push);
            let folders = mailboxes
                .iter()
                .map(|mailbox| {
                    let key = mailbox.id.key().as_str().to_owned();
                    SyncFolderRow {
                        subscribed: is_push && subscribed.contains(key.as_str()),
                        name: mailbox.name.clone(),
                        role: folder_role(mailbox),
                        key,
                    }
                })
                .collect();
            rows.push(AccountSyncRow {
                account_id: account.id.as_str().to_owned(),
                email: account.identity.email.clone(),
                idle_supported,
                strategy: kind(eff.strategy),
                poll_interval_mins: eff.poll_interval_mins,
                sync_depth_months,
                message_size_limit_mb: self
                    .stored_size_override(account.id.as_str())
                    .map_or_else(default_size_limit_mb, u16::from),
                at_push_limit: eff.push_folders.len() >= MAX_PUSH_FOLDERS,
                folders,
            });
        }
        SyncSettingsSnapshot {
            accounts: rows,
            max_push_folders: u8::try_from(MAX_PUSH_FOLDERS).unwrap_or(u8::MAX),
            poll_intervals: POLL_INTERVALS.to_vec(),
            sync_depths: SYNC_DEPTHS.to_vec(),
            message_size_limits_mb: MESSAGE_SIZE_LIMITS_MB.to_vec(),
        }
    }

    /// The behaviour currently in effect for an account: the seed every mutator starts
    /// from, so changing one field (e.g. the interval) never drops the implicit defaults
    /// (the Inbox push set). `None` for an unknown account id.
    pub(crate) async fn effective_for(&self, id: &str) -> Option<EffectiveSync> {
        let account_id = AccountId::try_from(id).ok()?;
        let account = self.account_handle(&account_id).await?;
        let idle_supported = account
            .providers
            .iter()
            .any(|p| p.connection_info().capabilities.idle());
        let mailboxes = self.engine.mailboxes(&account.id).await.unwrap_or_default();
        let default_push = inbox_default(&mailboxes);
        let stored = self
            .sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .get(id);
        Some(effective(stored.as_ref(), idle_supported, &default_push))
    }

    /// Switches an account between push and poll. The companion folder set / interval are
    /// preserved from the current effective state, so the user can flip back without
    /// re-picking. Persists and signals [`Surface::Settings`]. The bindings restart the
    /// account's background work after this.
    pub async fn set_sync_strategy(&self, id: &str, strategy: SyncStrategyKind) {
        let Some(eff) = self.effective_for(id).await else {
            return;
        };
        self.store_entry(
            id,
            AccountSyncSettings {
                strategy: to_strategy(strategy),
                push_folders: eff.push_folders,
                poll_interval_mins: eff.poll_interval_mins,
                sync_depth: self.stored_depth_override(id),
                message_size_limit: self.stored_size_override(id),
            },
        );
    }

    /// Sets an account's poll interval (snapped to the allowed set), leaving its strategy
    /// and push folders untouched. Persists and signals [`Surface::Settings`].
    pub async fn set_poll_interval(&self, id: &str, minutes: u16) {
        let Some(eff) = self.effective_for(id).await else {
            return;
        };
        self.store_entry(
            id,
            AccountSyncSettings {
                strategy: eff.strategy,
                push_folders: eff.push_folders,
                poll_interval_mins: snap_poll_interval(minutes),
                sync_depth: self.stored_depth_override(id),
                message_size_limit: self.stored_size_override(id),
            },
        );
    }

    /// Subscribes or unsubscribes one folder for push. Subscribing past
    /// [`MAX_PUSH_FOLDERS`] is ignored (clients also disable the control at the limit via
    /// [`AccountSyncRow::at_push_limit`]). Toggling implies push, so the account is set to
    /// push (a no-op if it already is). Persists and signals [`Surface::Settings`].
    pub async fn set_push_folder(&self, id: &str, folder: &str, subscribed: bool) {
        let Some(eff) = self.effective_for(id).await else {
            return;
        };
        let mut folders = eff.push_folders;
        if subscribed {
            if !folders.iter().any(|existing| existing == folder) {
                folders.push(folder.to_owned());
            }
            folders = cap_push_folders(&folders);
        } else {
            folders.retain(|existing| existing != folder);
        }
        self.store_entry(
            id,
            AccountSyncSettings {
                strategy: SyncStrategy::Push,
                push_folders: folders,
                poll_interval_mins: eff.poll_interval_mins,
                sync_depth: self.stored_depth_override(id),
                message_size_limit: self.stored_size_override(id),
            },
        );
    }

    /// Sets an account's sync depth (its explicit per-account override) as a month count
    /// (`0` = all mail), leaving its strategy / folders / interval untouched. Persists and
    /// signals [`Surface::Settings`]. This only changes the stored setting; user-facing depth
    /// changes go through [`update_account_sync_depth`](Self::update_account_sync_depth), which
    /// re-snapshots mail under the new window.
    pub async fn set_account_sync_depth(&self, id: &str, months: u16) {
        let Some(eff) = self.effective_for(id).await else {
            return;
        };
        self.store_entry(
            id,
            AccountSyncSettings {
                strategy: eff.strategy,
                push_folders: eff.push_folders,
                poll_interval_mins: eff.poll_interval_mins,
                sync_depth: Some(SyncDepth::from(months)),
                message_size_limit: self.stored_size_override(id),
            },
        );
    }

    /// Applies a user-facing sync-depth change and immediately reconciles the account's mail under
    /// the new window. Clearing mail cursors is required both ways: widening must backfill older
    /// mail rather than taking a delta, and narrowing must produce a fresh snapshot that tombstones
    /// rows now outside the window.
    pub async fn update_account_sync_depth(&self, id: &str, months: u16) {
        let Ok(account_id) = AccountId::try_from(id) else {
            return;
        };
        if self.account_handle(&account_id).await.is_none() {
            return;
        }
        let before = self.effective_sync_depth(id);
        self.set_account_sync_depth(id, months).await;
        let after = self.effective_sync_depth(id);
        if before == after {
            return;
        }
        log::info!(
            "sync-depth: changed from {} to {}; starting account resnapshot",
            depth_label(before),
            depth_label(after),
        );
        self.reset_on_demand_folders();
        let narrowed = is_narrower(after, before);
        if narrowed {
            // Enforce the new depth **before** asking the server anything. "Keep less of my
            // mail on this device" is a decision about this device, and a user who narrows
            // depth to free space on a plane must not have to wait for a network to get it.
            // The re-snapshot below reaches the same state when it can; this is what makes
            // that a reconciliation rather than the only path.
            self.prune_account_to_depth(&account_id, after).await;
        }
        self.resync_account_after_depth_change(&account_id).await;
        if narrowed {
            self.reclaim_freed_space("sync-depth").await;
        }
    }

    /// Drops the account's now-out-of-window mail locally, with no provider round trip.
    async fn prune_account_to_depth(&self, id: &AccountId, depth: SyncDepth) {
        let acct = self.account_ordinal(id).await;
        match self
            .engine
            .prune_account_mail_outside_window(id, sync_window(depth))
            .await
        {
            Ok(report) => {
                self.invalidate_list_cache();
                self.rebuild_snapshot().await;
                log::info!(
                    "sync-depth[a{acct}]: removed {} message(s) now outside the window",
                    report.messages_removed,
                );
            }
            Err(err) => log::warn!("sync-depth[a{acct}]: local prune failed: {err}"),
        }
    }

    /// Reclaims what a bulk removal freed: the cached raw sources on disk, then the
    /// database pages. Both are needed: the files are the larger half, and neither is
    /// released by the delete that orphaned it.
    pub(crate) async fn reclaim_freed_space(&self, label: &str) {
        match self.engine.sweep_unreferenced_blobs().await {
            Ok(report) => log::info!(
                "{label}: reclaimed {} cached message(s), {} MB",
                report.blobs_removed,
                report.bytes_reclaimed / 1_000_000,
            ),
            Err(err) => log::warn!("{label}: blob sweep failed: {err}"),
        }
        if let Err(err) = self.engine.vacuum().await {
            log::warn!("{label}: vacuum failed: {err}");
        }
    }

    /// The sync depth **in effect** for `id` (its override, else the product default); read by
    /// the bindings' connect paths to window each account's mail to its own depth.
    #[must_use]
    pub fn effective_sync_depth(&self, id: &str) -> SyncDepth {
        self.sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .effective_depth(id)
    }

    /// Drops one account's persisted sync settings. Called when the account is removed so a
    /// future add of the same login gets the new-account default again.
    pub(crate) fn remove_sync_settings(&self, id: &str) {
        self.sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .remove_entry(id);
        self.observer.surface_changed(Surface::Settings);
    }

    /// The account's stored explicit depth override (or `None`), read to preserve it when a
    /// non-depth field of its settings changes.
    pub(crate) fn stored_depth_override(&self, id: &str) -> Option<SyncDepth> {
        self.sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .depth_override(id)
    }

    /// Persists one account's settings and signals the settings surface.
    pub(crate) fn store_entry(&self, id: &str, entry: AccountSyncSettings) {
        self.sync_settings
            .lock()
            .expect("sync-settings mutex poisoned")
            .set_entry(id, entry);
        self.observer.surface_changed(Surface::Settings);
    }
}

fn is_narrower(next: SyncDepth, previous: SyncDepth) -> bool {
    depth_months(next) < depth_months(previous)
}

fn depth_months(depth: SyncDepth) -> u32 {
    match depth {
        SyncDepth::Months(months) => u32::from(months),
        SyncDepth::AllTime => u32::MAX,
    }
}

fn depth_label(depth: SyncDepth) -> String {
    match depth {
        SyncDepth::Months(months) => format!("{months} months"),
        SyncDepth::AllTime => "all mail".to_owned(),
    }
}
