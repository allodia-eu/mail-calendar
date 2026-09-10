//! The **targeted** folder refresh: one named folder, on demand or on a push.
//!
//! Its counterpart is `sync`, the account-wide pass. The distinction is a round
//! trip, not a tidiness: a pass lists the account's folders first, and on a server without
//! `LIST-STATUS` that is a `STATUS` per folder; thirteen extra round trips to serve a
//! notification that already named the one folder that changed. This half discovers nothing and
//! does no account-level work, and reaches the engine through `refresh_folders` rather than
//! `sync_mail`.

use std::time::Instant;

use engine_api::{AccountId, MailboxRole, Provider};

use crate::App;

impl<P: Provider> App<P> {
    /// Re-syncs a single watched folder on an IMAP `IDLE` push notification, then rebuilds the
    /// list so new mail appears near-instantly. It starts hidden and shows progress only if mail
    /// is actually downloaded. The watch runs on its own
    /// connection, so this connects a short-lived sync provider for the folder via the host
    /// [`MailboxConnector`](crate::MailboxConnector), streams it, derives threads, and drops
    /// the provider. A no-op without a connector (the demo / tests) or if the folder can't
    /// be connected (a transient blip: the next notification or poll catches it).
    pub async fn sync_watched_folder(&self, id: &AccountId, folder_key: &str) {
        if self.resync_folder(id, folder_key, "watch").await {
            // Warm the just-delivered bodies right away (after the list shows the new rows),
            // so a push-notified message opens instantly (and offline) by the time the user
            // taps it. Uses the account's own mail connection, not the sync's short-lived
            // provider.
            self.prefetch_account_bodies(id).await;
        }
    }

    /// The one-folder re-sync behind [`sync_watched_folder`](Self::sync_watched_folder), and
    /// the body-warm pass's conflict recovery ([`prefetch`](crate::prefetch)), which must not
    /// re-enter the warm pass (that would recurse). Connects a short-lived sync provider for
    /// the folder via the host [`MailboxConnector`](crate::MailboxConnector), streams it,
    /// derives threads, and drops the provider; returns whether the folder actually changed.
    /// A no-op (`false`) offline, without a connector (the demo / tests), or if the folder
    /// can't be connected (a transient blip: the next notification or poll catches it).
    pub(crate) async fn resync_folder(
        &self,
        id: &AccountId,
        folder_key: &str,
        label: &'static str,
    ) -> bool {
        // A push notification can't arrive while offline, but guard anyway so a racing
        // just-dropped connection doesn't attempt a doomed on-demand connect.
        if !self.is_online() {
            return false;
        }
        let Some(connector) = self.connector.as_ref() else {
            return false;
        };
        let depth = self.effective_sync_depth(id.as_str());
        let Some(provider) = connector.connect_folder(id, folder_key, depth).await else {
            return false;
        };
        let progress = self.begin_sync_labeled(false, true, 1, label);
        let tuning = self.sync_tuning_for(id);
        let acct = self.account_ordinal(id).await;
        let changed = {
            // A targeted refresh, not a pass: the notification already named the folder, so
            // re-listing the account's folders to serve it is a round trip that can tell us
            // nothing, and on a server without LIST-STATUS it is a round trip *per folder*.
            let report = self
                .engine
                .refresh_folders(core::slice::from_ref(&provider), id, tuning, &progress)
                .await;
            // The push path is the one a user notices, so it says what it did and how long it
            // took: a folder name would identify the user's mail, so it is the label that
            // distinguishes a watch from an on-demand open (`docs/logging.md`).
            match report.first_error() {
                Some(err) => log::warn!(
                    "refresh[a{acct}/{label}]: failed in {}ms: {err}",
                    report.elapsed.as_millis()
                ),
                None => log::info!(
                    "refresh[a{acct}/{label}]: +{} -{} in {}ms (fetch {}ms, derive {}ms, store {}ms)",
                    report.upserted(),
                    report.tombstoned(),
                    report.elapsed.as_millis(),
                    report
                        .folders
                        .first()
                        .map_or(0, |f| f.timing.fetching.as_millis()),
                    report
                        .folders
                        .first()
                        .map_or(0, |f| f.timing.deriving.as_millis()),
                    report
                        .folders
                        .first()
                        .map_or(0, |f| f.timing.storing.as_millis()),
                ),
            }
            report.upserted() + report.tombstoned() > 0
        };
        self.end_sync(&progress);
        // A watch sync that found nothing new (common at boot: the initial refresh already
        // synced this folder, so the watch's "sync once before trusting the watch" races it and
        // usually finds +0) must NOT drop the account cache and rebuild: that redundant
        // whole-account reload was a large part of the boot CPU + scroll jank. Do the expensive
        // work only when the folder actually changed.
        if changed {
            self.invalidate_list_cache();
            self.rebuild_snapshot().await;
        }
        drop(provider);
        changed
    }

    /// Re-syncs the folder on screen when the account pass that just ran could not reach it.
    ///
    /// An account pass syncs the providers bound when the account connected: INBOX and the
    /// folders the server tagged with SPECIAL-USE. Every other folder is downloaded once, by
    /// [`ensure_folder_synced`](Self::ensure_folder_synced), and nothing names it again: it is
    /// in no pass, and only the Inbox is watched. So standing in a mailing-list folder, every
    /// refresh; the pull, the poll tick, the return to network; left the list exactly as it was
    /// when the folder was opened, and a restart was the only way to see new mail in it.
    ///
    /// `pass_account` is the account the pass covered, or `None` for a pass over every account:
    /// one account's poll tick has no business connecting a folder of another.
    pub(crate) async fn refresh_open_folder(
        &self,
        pass_account: Option<&AccountId>,
        label: &'static str,
    ) {
        let open = {
            let scope = self.scope.lock().expect("scope mutex poisoned");
            scope
                .folder()
                .and_then(|key| Some((scope.account()?.clone(), key.to_owned())))
        };
        let Some((account, key)) = open else {
            return;
        };
        if pass_account.is_some_and(|id| id != &account) {
            return;
        }
        if self.is_eagerly_bound(&account, &key).await {
            return;
        }
        self.resync_folder(&account, &key, label).await;
    }

    /// Whether the account bound a provider to `key` when it connected: INBOX and the folders
    /// the server tagged with SPECIAL-USE (Sent/Drafts/Trash/Archive/Junk). Those sync with
    /// every account pass; every other folder is this module's business.
    ///
    /// Reads the folder's role from the (small) folder list; scanning every message was an
    /// O(N) stall on a large mailbox, on every folder open.
    async fn is_eagerly_bound(&self, account: &AccountId, key: &str) -> bool {
        self.engine
            .mailboxes(account)
            .await
            .unwrap_or_default()
            .iter()
            .find(|mailbox| mailbox.id.key().as_str() == key)
            .is_some_and(|mailbox| {
                matches!(
                    mailbox.role,
                    Some(
                        MailboxRole::Inbox
                            | MailboxRole::Sent
                            | MailboxRole::Drafts
                            | MailboxRole::Trash
                            | MailboxRole::Archive
                            | MailboxRole::Junk
                    )
                )
            })
    }

    /// Downloads a folder's mail on demand the first time it is opened, if it isn't synced
    /// already: the "sync the folder you open" path for custom/untagged folders. A no-op
    /// without a [`MailboxConnector`](crate::MailboxConnector) (the demo / tests).
    ///
    /// Returns whether mail was actually downloaded, so the caller can skip the rebuild that
    /// would only republish the snapshot it already has. Most folder opens land in one of the
    /// early returns below: the eager bind covers the role folders, and any folder opens at
    /// most once a session.
    pub(crate) async fn ensure_folder_synced(&self, account: &AccountId, key: &str) -> bool {
        let Some(connector) = self.connector.as_ref() else {
            return false;
        };
        // Attempt each folder at most once per session.
        let first_attempt = self
            .attempted_folders
            .lock()
            .expect("attempted-folders mutex poisoned")
            .insert((account.as_str().to_owned(), key.to_owned()));
        if !first_attempt {
            return false;
        }
        // Only a folder the eager bind skipped (a custom folder, or a role folder the server
        // didn't tag; e.g. an untagged Archive) needs an on-demand connection.
        if self.is_eagerly_bound(account, key).await {
            return false;
        }
        let connect_start = Instant::now();
        let depth = self.effective_sync_depth(account.as_str());
        let Some(provider) = connector.connect_folder(account, key, depth).await else {
            // The connect failed (a network blip / login timeout). Forget the attempt so
            // re-opening the folder tries again: a *transient* failure must not leave the
            // folder showing empty for the rest of the session.
            self.attempted_folders
                .lock()
                .expect("attempted-folders mutex poisoned")
                .remove(&(account.as_str().to_owned(), key.to_owned()));
            return false;
        };
        log::debug!(
            "on-demand: connected folder in {}ms",
            connect_start.elapsed().as_millis(),
        );
        // Opening an unsynced folder is an explicit, user-awaited download; show the bar.
        let progress = self.begin_sync_labeled(true, true, 1, "on-demand");
        let sync_start = Instant::now();
        let tuning = self.sync_tuning_for(account);
        {
            // The user opened this folder and is waiting on it; the folder list is not what
            // they asked for.
            let _ = self
                .engine
                .refresh_folders(core::slice::from_ref(&provider), account, tuning, &progress)
                .await;
        }
        self.end_sync(&progress);
        self.invalidate_list_cache();
        log::info!(
            "on-demand: folder synced in {}ms",
            sync_start.elapsed().as_millis(),
        );
        // No body prefetch here: `SelectFolder` awaits this method *before* it rebuilds the
        // snapshot, so a warm pass would hold the just-opened folder's rows off screen. The
        // folder's messages are in the mail index now, so the next poll tick's prefetch warms
        // them like any other synced mail.
        // The connection is dropped here: the folder's mail is now cached in the store and
        // stays visible. Keeping it up to date afterwards is
        // [`refresh_open_folder`](Self::refresh_open_folder)'s job, which connects again for as
        // long as the folder is the one on screen.
        drop(provider);
        true
    }

    /// Forgets which folders have been on-demand synced this session, so they re-sync the next
    /// time they are opened; used after a sync-depth change so opened folders get the new
    /// per-sync window.
    pub fn reset_on_demand_folders(&self) {
        self.attempted_folders
            .lock()
            .expect("attempted-folders mutex poisoned")
            .clear();
    }
}
