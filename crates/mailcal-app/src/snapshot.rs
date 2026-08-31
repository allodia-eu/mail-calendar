//! Mailbox-list snapshot building: project the accounts in view into the
//! [`MailboxListSnapshot`] the host renders and signal [`Surface::MailboxList`]. Split out of
//! `lib.rs` to keep it under the 500-line limit; an `impl App` block reusing the runtime's
//! fields. Search has its own projection in [`snapshot_search`](crate::snapshot_search).
//!
//! **The unified inbox and one account's folder are the same projection.** They differ only in
//! which accounts the rows come from and which of those rows count as *in view*: a predicate
//! ([`InView`]) rather than a second code path. Two projections meant two answers to "what is a
//! conversation here", and the second one quietly dropped each conversation's out-of-view members,
//! which is how a thread row collapsed to a flat row and lost its expansion.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use engine_api::{AccountId, MailListRow, MailboxRole, Provider};
use mailcal_viewmodel::{
    AccountFolderRow, AccountMessage, AccountRow, MailboxListSnapshot, ViewMode, view,
};

use crate::{App, CachedRows};

/// Which of the rows in view actually belong to the list being shown.
///
/// The rows themselves come from the accounts in view; this decides which of them the flat list
/// shows and which conversations the threaded list lists. Out-of-view rows are still projected;
/// they are what completes a conversation with the owner's Sent replies.
pub(crate) enum InView {
    /// Every message of the accounts in view counts: an account's own all-mail.
    Everything,
    /// Only messages filed in the named mailbox **of their own account**: the selected folder for
    /// one account, or each account's Inbox for the unified view.
    Mailbox(HashMap<String, String>),
}

impl InView {
    /// The unified view: each account's own INBOX is what counts as in view.
    pub(crate) fn inboxes(by_account: HashMap<String, String>) -> Self {
        Self::Mailbox(by_account)
    }

    /// One account's view: the selected folder, or everything when its all-mail is showing.
    pub(crate) fn folder(account: &AccountId, folder: Option<&str>) -> Self {
        match folder {
            None => Self::Everything,
            Some(key) => Self::Mailbox(HashMap::from([(
                account.as_str().to_owned(),
                key.to_owned(),
            )])),
        }
    }

    pub(crate) fn holds(&self, row: &MailListRow) -> bool {
        match self {
            Self::Everything => true,
            Self::Mailbox(by_account) => by_account
                .get(row.account.as_str())
                .is_some_and(|wanted| row.mailboxes.iter().any(|id| id.as_str() == wanted)),
        }
    }
}

impl<P: Provider> App<P> {
    /// Reads the accounts in view and projects them into the mailbox-list snapshot; ranked
    /// search when a query is active, else the unified all-inboxes (no account selected) or one
    /// account's folder view; then signals [`Surface::MailboxList`].
    pub(super) async fn rebuild_snapshot(&self) {
        let start = Instant::now();
        let account_rows = self.account_rows().await;
        // One read of one lock: the account and the folder are one value, so a snapshot can
        // never pair a folder key with an account that was not showing it.
        let scope = self.scope.lock().expect("scope mutex poisoned").clone();
        let query = self
            .search_query
            .lock()
            .expect("search mutex poisoned")
            .clone();
        let mode = *self.view_mode.lock().expect("view-mode mutex poisoned");
        let limit = self.visible_limit();
        let window = self.load_window();

        let mut snapshot = if let Some(query) = query {
            self.search_snapshot(scope.account(), scope.folder(), &query, &account_rows)
                .await
        } else {
            self.mailbox_snapshot(
                scope.account(),
                scope.folder(),
                mode,
                &account_rows,
                limit,
                window,
            )
            .await
        };
        let rows = snapshot.rows.len();
        let total = snapshot.total;
        self.restamp_expansion(&mut snapshot);
        // Fills in the photos already known, and reports the senders nobody has looked up
        // yet. A map read per row: the fetch that answers the rest happens after the
        // snapshot is published, so a face arriving never delays the list appearing.
        let unresolved = self.attach_photos(&mut snapshot);
        self.mailbox_list.publish(snapshot);
        log::info!(
            "rebuild_snapshot: {rows} row(s) of {total} in {}ms",
            start.elapsed().as_millis(),
        );
        self.resolve_sender_photos(unresolved).await;
    }

    /// The mailbox list for the current selection.
    ///
    /// `selected` names one account's folder view; `None` is the unified "all inboxes", where
    /// every account contributes and each one's INBOX is what counts as in view. Either way the
    /// rows are one ordered read across the accounts in view, and the flat list shows the in-view
    /// ones while the threaded list shows the conversations that touch them; each carrying its
    /// other messages (the owner's Sent replies) from the rest of the mailbox.
    async fn mailbox_snapshot(
        &self,
        selected: Option<&AccountId>,
        folder: Option<&str>,
        mode: ViewMode,
        account_rows: &[AccountRow],
        limit: usize,
        window: usize,
    ) -> MailboxListSnapshot {
        let (accounts, in_view) = self.view_scope(selected, folder).await;
        let items = self
            .list_items(&accounts, &in_view, account_rows, mode, window)
            .await;
        let folders = match selected {
            Some(account) => self.engine.mailboxes(account).await.unwrap_or_default(),
            None => Vec::new(),
        };
        let account_folders = self.all_account_folders(account_rows).await;
        view::build(
            &items,
            &folders,
            account_rows,
            account_folders,
            selected.map(AccountId::as_str),
            folder,
            mode,
            limit,
        )
    }

    /// The accounts the shown list draws from, and which of their rows are in view.
    async fn view_scope(
        &self,
        selected: Option<&AccountId>,
        folder: Option<&str>,
    ) -> (Vec<AccountId>, InView) {
        let Some(account) = selected else {
            // The unified view: every account, each one's INBOX in view. An account with no
            // inbox key contributes rows but nothing in view, so nothing of it is shown.
            let mut inboxes = HashMap::new();
            let accounts = self.account_ids().await;
            for id in &accounts {
                if let Some(inbox) = self.inbox_key(id).await {
                    inboxes.insert(id.as_str().to_owned(), inbox);
                }
            }
            return (accounts, InView::inboxes(inboxes));
        };
        (vec![account.clone()], InView::folder(account, folder))
    }

    /// The projected items for the current view: the windowed rows, each tagged with whether it
    /// is in view and whether the owner sent it; **plus**, in the threaded view, every *other*
    /// member of the conversations the window touches, pulled from the store's thread index
    /// regardless of age.
    ///
    /// That completion is the point of the design: the date window decides which conversations
    /// appear, but an expanded thread still shows its whole history (a years-old reply included)
    /// even when older members fall outside the window. The flat view needs no completion; it
    /// lists messages, not conversations.
    async fn list_items(
        &self,
        accounts: &[AccountId],
        in_view: &InView,
        account_rows: &[AccountRow],
        mode: ViewMode,
        window: usize,
    ) -> Vec<AccountMessage> {
        let base = self.cached_rows(accounts, window).await;
        let mut items: Vec<AccountMessage> = base
            .iter()
            .map(|row| account_message(in_view, account_rows, row))
            .collect();
        if mode != ViewMode::Threaded {
            return items;
        }
        // Complete each windowed conversation with the members that fall outside the window
        // (older replies, the owner's Sent copies filed elsewhere); in **one** indexed read for
        // all the shown threads, never one read per thread.
        // Keyed on `(account, key)`: a provider key is unique within an account, not across the
        // several a unified list spans.
        let present: HashSet<(&str, &str)> = base
            .iter()
            .map(|row| (row.account.as_str(), row.mail.key.as_str()))
            .collect();
        let hidden = self.pending_hidden_keys();
        let threads: HashSet<&str> = base
            .iter()
            .filter_map(|row| {
                row.mail
                    .thread_id
                    .as_ref()
                    .map(engine_api::ThreadId::as_str)
            })
            .collect();
        let completion_start = Instant::now();
        let extra = self
            .engine
            .mail_on_threads(accounts, threads.iter().copied())
            .await
            .unwrap_or_default();
        log::debug!(
            "thread completion: {} thread(s) -> {} member(s) in {}ms",
            threads.len(),
            extra.len(),
            completion_start.elapsed().as_millis(),
        );
        for member in extra {
            let key = (member.account.as_str(), member.mail.key.as_str());
            if present.contains(&key) || hidden.contains(&(key.0.to_owned(), key.1.to_owned())) {
                continue;
            }
            items.push(account_message(in_view, account_rows, &Arc::new(member)));
        }
        items
    }

    /// Re-reads each account's folder-tree expansion **at publish time**, overwriting whatever
    /// the projection captured when it began.
    ///
    /// A rebuild spans several `await`s (store reads per account), so one that started before the
    /// user's chevron finishes after it and would publish the expansion as it was *then*;
    /// springing the tree back open a beat after they shut it. During a sync, when rebuilds are
    /// frequent, that is most of the time. The same shape as the contacts-search generation
    /// counter: the pass that started earlier must not win by finishing later.
    ///
    /// Cheap enough to do unconditionally; it is an in-memory set lookup per account, and the
    /// alternative (a generation counter over the whole snapshot) would pay for a race this state
    /// is the only writer of.
    fn restamp_expansion(&self, snapshot: &mut MailboxListSnapshot) {
        for row in &mut snapshot.accounts {
            row.expanded = self.account_expanded(&row.id);
        }
    }

    /// Fetches every account's sorted folder list in `account_rows` order: for the
    /// navigation drawer, which shows all accounts simultaneously.
    async fn all_account_folders(&self, account_rows: &[AccountRow]) -> Vec<AccountFolderRow> {
        let mut out = Vec::with_capacity(account_rows.len());
        for row in account_rows {
            let Ok(id) = AccountId::try_from(row.id.as_str()) else {
                continue;
            };
            let mailboxes = self.engine.mailboxes(&id).await.unwrap_or_default();
            out.push(AccountFolderRow {
                account_id: row.id.clone(),
                folders: mailcal_viewmodel::sorted_folder_rows(&mailboxes),
            });
        }
        out
    }

    /// Every optimistically-removed `(account, key)`; just archived or deleted, the move not yet
    /// reflected by a sync. Read-only sibling of the prune in [`cached_rows`](Self::cached_rows),
    /// so thread completion honours the same hiding without re-running the prune.
    pub(crate) fn pending_hidden_keys(&self) -> HashSet<(String, String)> {
        self.pending_removals
            .lock()
            .expect("pending-removals mutex poisoned")
            .clone()
    }

    /// The windowed rows for projection: the [`load_rows`](Self::load_rows) set with any
    /// **optimistically removed** rows filtered out. A hidden key the store no longer reports is
    /// pruned here (once the move lands the hint has done its job) so the set self-prunes and a
    /// message that legitimately returns is shown again. The hot path (nothing hidden) returns
    /// the shared cache untouched.
    pub(crate) async fn cached_rows(
        &self,
        accounts: &[AccountId],
        window: usize,
    ) -> Arc<Vec<Arc<MailListRow>>> {
        let base = self.load_rows(accounts, window).await;
        let mut removals = self
            .pending_removals
            .lock()
            .expect("pending-removals mutex poisoned");
        if removals.is_empty() {
            return base;
        }
        // Prune confirmed removals: once the store stops reporting a hidden key, the move has
        // landed, so drop the hint (a failed edit was already un-hidden by its caller). Only the
        // accounts actually read can be judged; one absent from this window says nothing.
        let read: HashSet<&str> = accounts.iter().map(AccountId::as_str).collect();
        let present: HashSet<(&str, &str)> = base
            .iter()
            .map(|row| (row.account.as_str(), row.mail.key.as_str()))
            .collect();
        removals.retain(|(account, key)| {
            !read.contains(account.as_str()) || present.contains(&(account.as_str(), key.as_str()))
        });
        let hidden: HashSet<(String, String)> = removals.clone();
        drop(removals);
        Arc::new(
            base.iter()
                .filter(|row| {
                    !hidden.contains(&(
                        row.account.as_str().to_owned(),
                        row.mail.key.as_str().to_owned(),
                    ))
                })
                .cloned()
                .collect(),
        )
    }

    /// The newest `window` rows across `accounts`, from the in-memory cache; one ordered store
    /// read on first use, reused for every later navigation of the same view.
    ///
    /// A cached load is reused when it spans exactly these accounts at a **deeper or equal**
    /// window (a wider window is a superset: the extra newest rows are harmless, the view
    /// truncates); a different set of accounts, or a deeper [`Intent::ShowMore`] than the cache
    /// holds, reloads. [`invalidate_list_cache`](Self::invalidate_list_cache) drops it after a
    /// sync so it never serves stale rows.
    async fn load_rows(&self, accounts: &[AccountId], window: usize) -> Arc<Vec<Arc<MailListRow>>> {
        if let Some(cached) = self
            .row_cache
            .lock()
            .expect("row-cache mutex poisoned")
            .as_ref()
            .filter(|cached| cached.serves(accounts, window))
        {
            return Arc::clone(&cached.rows);
        }
        // Miss: snapshot the cache generation, then read from the store without holding the lock
        // across the await. This is one indexed read of exactly `window` projected rows; no
        // payload is opened to draw a list row. Each row is wrapped in its own `Arc` so a rebuild
        // pairs it with its view flags by bumping the `Arc` rather than deep-copying it.
        let generation = self.row_cache_generation.load(Ordering::SeqCst);
        let load_start = Instant::now();
        let rows: Arc<Vec<Arc<MailListRow>>> = Arc::new(
            self.engine
                .mail_window(accounts, window)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(Arc::new)
                .collect(),
        );
        log::debug!(
            "list rows: read {} row(s) at window {window} in {}ms (cache miss)",
            rows.len(),
            load_start.elapsed().as_millis(),
        );
        // If the store returned fewer rows than the window asked for, this IS everything the
        // accounts hold; cache it as satisfying an *unbounded* window, so scrolling deeper
        // reuses it instead of re-reading the same full set every step.
        let effective_window = if rows.len() < window {
            usize::MAX
        } else {
            window
        };
        // Only publish this load if no sync invalidated the cache while it was in flight;
        // otherwise a slow pre-sync read could land *after* the invalidation and resurrect stale
        // rows. On a mismatch, return the freshly-read rows for this one snapshot without caching
        // them; the sync that bumped the generation rebuilds from its own fresh read.
        if self.row_cache_generation.load(Ordering::SeqCst) == generation {
            *self.row_cache.lock().expect("row-cache mutex poisoned") = Some(CachedRows {
                accounts: accounts.to_vec(),
                window: effective_window,
                rows: Arc::clone(&rows),
            });
            // The store read landed, so the list is held in memory again and the live path may
            // splice into it once more.
            self.row_cache_dropped.store(false, Ordering::SeqCst);
        }
        rows
    }

    /// Drops the cached list so the next snapshot re-reads it; called after a sync commits new
    /// state. The read it forces is one indexed query for the window on screen, which is why this
    /// no longer needs to be per-account.
    pub(crate) fn invalidate_list_cache(&self) {
        self.row_cache_generation.fetch_add(1, Ordering::SeqCst);
        *self.row_cache.lock().expect("row-cache mutex poisoned") = None;
        // Mark it, don't just drop it: the live path must be able to tell "we dropped this" from
        // "we never loaded it"; see `App::row_cache_dropped`.
        self.row_cache_dropped.store(true, Ordering::SeqCst);
    }

    /// Whether the shown list's rows are currently held in memory.
    #[cfg(test)]
    pub(crate) fn list_cache_is_loaded(&self) -> bool {
        self.row_cache
            .lock()
            .expect("row-cache mutex poisoned")
            .is_some()
    }

    /// The provider key of `account`'s INBOX (resolved by role), or `None`.
    pub(crate) async fn inbox_key(&self, account: &AccountId) -> Option<String> {
        if let Some(key) = self
            .inbox_keys
            .lock()
            .expect("inbox-key mutex poisoned")
            .get(account.as_str())
            .cloned()
        {
            return Some(key);
        }
        let key = self
            .engine
            .mailboxes(account)
            .await
            .unwrap_or_default()
            .into_iter()
            .find(|mailbox| mailbox.role == Some(MailboxRole::Inbox))
            .map(|mailbox| mailbox.id.key().as_str().to_owned());
        if let Some(key) = &key {
            self.inbox_keys
                .lock()
                .expect("inbox-key mutex poisoned")
                .insert(account.as_str().to_owned(), key.clone());
        }
        key
    }
}

/// The owner address of the account `id`, from the switcher rows (its login identity), for
/// deciding which messages the account sent.
pub(crate) fn owner_email<'a>(account_rows: &'a [AccountRow], id: &str) -> Option<&'a str> {
    account_rows
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.email.as_str())
}

/// Pairs a projected row with the two flags only the app can decide: whether it is in the shown
/// view, and whether the account owner sent it. Shares the row by bumping its `Arc`.
pub(crate) fn account_message(
    in_view: &InView,
    account_rows: &[AccountRow],
    row: &Arc<MailListRow>,
) -> AccountMessage {
    let owner = owner_email(account_rows, row.account.as_str());
    AccountMessage {
        in_scope: in_view.holds(row),
        outgoing: is_outgoing(row, owner),
        row: Arc::clone(row),
    }
}

/// Whether `row` was sent by the account owner; its `From` address is the owner's own
/// (case-insensitive). Drives the conversation's "Sent" badge, and (reused by
/// [`background_sync`](crate::background_sync)) excludes the owner's own mail from new-mail
/// notifications.
pub(crate) fn is_outgoing(row: &MailListRow, owner: Option<&str>) -> bool {
    owner.is_some_and(|owner| {
        row.mail
            .from_addr
            .as_deref()
            .is_some_and(|from| from.eq_ignore_ascii_case(owner))
    })
}
