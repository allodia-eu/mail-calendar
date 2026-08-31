//! Live mailbox-list updates from streamed sync commits.
//!
//! The async snapshot builder remains the authoritative path. This module handles the
//! synchronous [`SyncObserver`](engine_api::SyncObserver) callback by splicing the commit's
//! rows into the app's cached list and re-projecting the currently visible non-search view from
//! it, without re-reading the store.
//!
//! **The fast path may only publish a list it can build completely.** It re-projects the *whole*
//! visible list (every account of the unified inbox, not just the commit's) so a cache dropped
//! mid-pass (a reconcile pass tombstones rows it cannot name) would leave the list missing rows
//! until the authoritative rebuild restored them a second later. On screen that is the entire
//! list flashing to a shorter one and back. So a dropped cache is neither seeded from a delta nor
//! projected over: the update is skipped and the current snapshot stays up. See `App::row_cache`
//! and `tests_live_mailbox`.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
};

use engine_api::{AccountId, MailListRow, Message, Provider, ProviderKey, SyncCommit};
use mailcal_viewmodel::{AccountMessage, AccountRow, view};

use crate::{
    App, CachedRows,
    snapshot::{InView, account_message},
};

impl<P: Provider> App<P> {
    /// Applies one streamed mail commit to the live cache and republishes the visible list.
    ///
    /// Enumerated upserts/removals are safe to splice immediately. Present-set tombstones
    /// only report a count, not keys, so those still fall back to the final authoritative
    /// rebuild after the sync pass.
    pub(crate) fn apply_live_mailbox_commit(&self, commit: &SyncCommit<'_>) -> bool {
        self.apply_live_mail_delta(
            commit.scope.account(),
            commit.upserted,
            commit.removed,
            commit.tombstoned,
        )
    }

    /// [`apply_live_mailbox_commit`](Self::apply_live_mailbox_commit) over the parts of a commit,
    /// so the rule can be driven from a test; `SyncCommit` is `#[non_exhaustive]`, so a test
    /// cannot build one.
    pub(crate) fn apply_live_mail_delta(
        &self,
        account: &AccountId,
        upserted: &[Message],
        removed: &[ProviderKey],
        tombstoned: usize,
    ) -> bool {
        if (upserted.is_empty() && removed.is_empty() && tombstoned == 0) || tombstoned > 0 {
            if tombstoned > 0 {
                self.invalidate_list_cache();
            }
            return false;
        }
        if !self.splice_live_rows(account, upserted, removed) {
            return false;
        }
        self.remove_confirmed_pending(account, removed);
        self.rebuild_live_mailbox_snapshot()
    }

    /// Splices one account's commit into the cached list, answering whether the cache could take
    /// it.
    ///
    /// A dropped cache is not an empty mailbox. Seeding one from this commit alone would make the
    /// whole list *look* like the delta; leave it dropped and let the store read that follows the
    /// pass reload it. A commit for an account the shown list does not span changes nothing on
    /// screen either, so it is likewise no reason to republish.
    fn splice_live_rows(
        &self,
        account: &AccountId,
        upserted: &[Message],
        removed: &[ProviderKey],
    ) -> bool {
        let removed: HashSet<&str> = removed.iter().map(ProviderKey::as_str).collect();
        let view_accounts = self.live_view_accounts();
        let window = self.load_window();
        let mut cache = self.row_cache.lock().expect("row-cache mutex poisoned");
        if cache.is_none() {
            if self.row_cache_dropped.load(Ordering::SeqCst) {
                return false;
            }
            // Never loaded: this commit is all there is to show, and showing it as it streams is
            // exactly what this path exists for.
            *cache = Some(CachedRows::empty(view_accounts, window));
        }
        let Some(cached) = cache.as_mut() else {
            return false;
        };
        if !cached.spans(account) {
            return false;
        }
        self.row_cache_generation.fetch_add(1, Ordering::SeqCst);
        let mut rows: Vec<Arc<MailListRow>> = cached
            .rows
            .iter()
            .filter(|row| {
                row.account.as_str() != account.as_str() || !removed.contains(row.mail.key.as_str())
            })
            .cloned()
            .collect();
        for message in upserted {
            // Projected here, through the engine's own projection, so a row spliced from a
            // stream and a row read back from the store are the same row.
            let mut row = MailListRow::project(account, message);
            if let Some(existing) = rows
                .iter()
                .position(|cached| cached.account == row.account && cached.mail.key == row.mail.key)
            {
                // The two columns no provider supplies: the thread the engine derived, and the
                // preview a provider with no server snippet has none of. A message re-sent whole
                // carries `None` for each, meaning "nothing to say", and the store keeps what it
                // holds rather than blanking them, so a row spliced from the stream has to do the
                // same or the list disagrees with the store it is standing in for. Dropping the
                // thread tears the open conversation into one row per message and puts it back a
                // moment later.
                let cached = &rows[existing].mail;
                row.mail.thread_id = row.mail.thread_id.or_else(|| cached.thread_id.clone());
                row.mail.preview = row.mail.preview.or_else(|| cached.preview.clone());
                rows[existing] = Arc::new(row);
            } else {
                rows.push(Arc::new(row));
            }
        }
        rows.sort_by(|a, b| {
            Reverse(a.mail.date_utc)
                .cmp(&Reverse(b.mail.date_utc))
                .then_with(|| {
                    (a.account.as_str(), a.mail.key.as_str())
                        .cmp(&(b.account.as_str(), b.mail.key.as_str()))
                })
        });
        if cached.window != usize::MAX {
            rows.truncate(cached.window);
        }
        cached.rows = Arc::new(rows);
        true
    }

    fn remove_confirmed_pending(&self, account: &AccountId, removed: &[ProviderKey]) {
        if removed.is_empty() {
            return;
        }
        let account = account.as_str();
        let removed: HashSet<&str> = removed.iter().map(ProviderKey::as_str).collect();
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .retain(|(acct, key)| acct != account || !removed.contains(key.as_str()));
    }

    fn rebuild_live_mailbox_snapshot(&self) -> bool {
        if self
            .search_query
            .lock()
            .expect("search mutex poisoned")
            .is_some()
        {
            return false;
        }
        // One read of one lock, so the account and the folder on screen are always the same
        // scope; never one half of a selection another task is still writing.
        let scope = self.scope.lock().expect("scope mutex poisoned").clone();
        let mode = *self.view_mode.lock().expect("view-mode mutex poisoned");
        let limit = self.visible_limit();
        let account_rows = self.live_account_rows();
        let current = self.mailbox_list.get();
        let in_view = match scope.account() {
            Some(account) => InView::folder(account, scope.folder()),
            None => InView::inboxes(self.live_inbox_keys()),
        };
        let Some(items) = self.live_items(&in_view, &account_rows) else {
            return false;
        };
        let mut snapshot = view::build(
            &items,
            &[],
            &account_rows,
            vec![],
            scope.account().map(AccountId::as_str),
            scope.folder(),
            mode,
            limit,
        );
        // The live path rebuilds the *rows* from the cached list and re-reads no folders, so it
        // carries the sidebar's data over wholesale. `unified_unread` is part of that data, not
        // part of the rebuild: it is derived from `account_folders`, which was just passed in
        // empty, so leaving it computed would blank the All Inboxes badge on every optimistic
        // update and restore it a beat later when the authoritative rebuild lands.
        snapshot.folders.clone_from(&current.folders);
        snapshot
            .account_folders
            .clone_from(&current.account_folders);
        snapshot.unified_unread = current.unified_unread;
        if snapshot == current {
            return false;
        }
        self.mailbox_list.publish(snapshot);
        true
    }

    /// The accounts the shown list draws from, resolved without an `await`: the live path runs
    /// inside a synchronous observer callback and cannot read the store.
    fn live_view_accounts(&self) -> Vec<AccountId> {
        if let Some(account) = self.scope.lock().expect("scope mutex poisoned").account() {
            return vec![account.clone()];
        }
        self.live_account_rows()
            .iter()
            .filter_map(|row| AccountId::try_from(row.id.as_str()).ok())
            .collect()
    }

    fn live_account_rows(&self) -> Vec<AccountRow> {
        self.accounts.try_read().map_or_else(
            |_| self.mailbox_list.get().accounts.clone(),
            |accounts| {
                accounts
                    .iter()
                    .map(|account| AccountRow {
                        id: account.id.as_str().to_owned(),
                        email: account.identity.email.clone(),
                        expanded: self.account_expanded(account.id.as_str()),
                    })
                    .collect()
            },
        )
    }

    fn live_inbox_keys(&self) -> HashMap<String, String> {
        self.inbox_keys
            .lock()
            .expect("inbox-key mutex poisoned")
            .clone()
    }

    /// The cached rows projected for the shown view, or `None` when the list is not in memory.
    ///
    /// The threaded view is **not** completed here: completion is a store read, and the live path
    /// exists precisely to avoid one. A conversation whose out-of-window members are missing for
    /// a beat still renders; the authoritative rebuild that follows the pass restores them.
    fn live_items(
        &self,
        in_view: &InView,
        account_rows: &[AccountRow],
    ) -> Option<Vec<AccountMessage>> {
        let cache = self.row_cache.lock().expect("row-cache mutex poisoned");
        let cached = cache.as_ref()?;
        let hidden = self.live_hidden_keys();
        Some(
            cached
                .rows
                .iter()
                .filter(|row| {
                    !hidden.contains(&(
                        row.account.as_str().to_owned(),
                        row.mail.key.as_str().to_owned(),
                    ))
                })
                .map(|row| account_message(in_view, account_rows, row))
                .collect(),
        )
    }

    fn live_hidden_keys(&self) -> HashSet<(String, String)> {
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .clone()
    }
}
