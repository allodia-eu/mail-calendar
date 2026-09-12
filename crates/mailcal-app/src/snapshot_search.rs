//! Search projected into the mailbox list: which accounts and folders the active
//! [`SearchScope`] covers, the Trash exclusion, and the newest-first merge across accounts.
//! Split out of `snapshot.rs` (which keeps the folder and unified-inbox projections) to stay
//! under the 500-line limit; an `impl App` block reusing the runtime's fields.
//!
//! The scope decides two separate things, and they are easy to conflate: **which accounts**
//! are searched, and **which of an account's folders** count. "All folders" searches every
//! configured account minus each one's Trash; "current folder" mirrors whatever the mailbox
//! list was showing; one folder of one account, that account's whole mailbox, or (in the
//! unified view) every account's Inbox.

use std::{collections::HashSet, sync::Arc, time::Instant};

use engine_api::{AccountId, MailListRow, Mailbox, MailboxRole, Provider, ProviderKey, ThreadId};
use mailcal_account::SyncDepth;
use mailcal_viewmodel::{
    AccountFolderRow, AccountMessage, AccountRow, MailboxListSnapshot, SearchHorizon, ViewMode,
    sorted_folder_rows, view,
};

use crate::{
    App, SEARCH_FETCH_LIMIT, SEARCH_LIMIT, SearchScope,
    snapshot::{is_outgoing, owner_email},
};

impl<P: Provider> App<P> {
    /// Full-text search projected into the mailbox list; every account the active scope
    /// covers, searched in turn, their hits pooled and shown **newest first**.
    ///
    /// The engine ranks each account's hits by relevance and caps them at
    /// [`SEARCH_FETCH_LIMIT`]; this reads that as a *candidate set*, filters it to the scope,
    /// and hands [`view::search_results`] the survivors to order by date and cap at
    /// [`SEARCH_LIMIT`].
    ///
    /// `mode` is the user's list grouping, which a search obeys like any other list: threaded,
    /// the matches are grouped into the conversations they belong to.
    pub(super) async fn search_snapshot(
        &self,
        selected_account: Option<&AccountId>,
        selected_folder: Option<&str>,
        query: &str,
        mode: ViewMode,
        account_rows: &[AccountRow],
    ) -> MailboxListSnapshot {
        let scope = self.search_state().scope;
        // Every account's folder list, read once: it decides the navigation drawer's rows,
        // which key is an account's Trash, and which is its Inbox.
        let mailboxes = self.account_mailboxes(account_rows).await;
        let mut hits = self
            .search_hits(
                query,
                scope,
                selected_account,
                selected_folder,
                account_rows,
                &mailboxes,
            )
            .await;
        if mode == ViewMode::Threaded {
            self.complete_search_threads(&mut hits, account_rows, &mailboxes)
                .await;
        }
        let account_folders: Vec<AccountFolderRow> = mailboxes
            .iter()
            .map(|(id, folders)| AccountFolderRow {
                account_id: id.as_str().to_owned(),
                folders: sorted_folder_rows(folders),
            })
            .collect();
        let mut snapshot =
            view::search_results(&hits, account_rows, account_folders, mode, SEARCH_LIMIT);
        // Keep the host's navigation on the searched scope: a search must not flip the account
        // switcher to "All Inboxes" or unhighlight the folder the user is standing in. The
        // client also renders its scope filter from these fields (they name the "this folder"
        // side of the toggle), and leaving search restores exactly this view.
        snapshot.selected_account = selected_account.map(|account| account.as_str().to_owned());
        snapshot.folders = selected_account
            .and_then(|account| {
                mailboxes
                    .iter()
                    .find(|(id, _)| id == account)
                    .map(|(_, folders)| sorted_folder_rows(folders))
            })
            .unwrap_or_default();
        snapshot.search_horizon = self.search_horizon(
            searched(scope, selected_account, selected_folder, &mailboxes)
                .into_iter()
                .map(|(account, _)| account),
        );
        snapshot.selected = selected_folder.map(str::to_owned);
        snapshot
    }

    /// The active search, and the generation that dates it, read together.
    pub(crate) fn search_state(&self) -> SearchState {
        self.search.lock().expect("search mutex poisoned").clone()
    }

    /// Replaces the active search and marks every snapshot rebuild now in flight as answering
    /// one the user has moved on from, so each is dropped instead of published.
    ///
    /// Intents are spawned, not queued, so typing a word runs a search per keystroke
    /// concurrently. They finish out of order and by wildly different margins: a two-letter
    /// prefix matches half the mailbox and resolves every hit from the store, so it can take a
    /// second and a half while the full query takes a tenth. Publishing unconditionally hands
    /// the screen to whichever finishes last, which is reliably the broadest and least useful
    /// one, and nothing corrects it until the next unrelated rebuild.
    ///
    /// Leaving search drops the scope filter with it, in the same write: the next search opens
    /// across everything rather than silently inheriting how the last one was narrowed, since a
    /// filter the user can no longer see is one they will not think of (`docs/search.md`, rule
    /// 6).
    pub(crate) fn set_search(&self, query: Option<String>) {
        let mut state = self.search.lock().expect("search mutex poisoned");
        if query.is_none() {
            state.scope = SearchScope::default();
        }
        state.query = query;
        state.generation = state.generation.wrapping_add(1);
    }

    /// Narrows or widens the active search. The query is unchanged; what is superseded is every
    /// answer to the old scope, so this bumps the generation exactly as a keystroke does.
    pub(crate) fn set_search_scope(&self, scope: SearchScope) {
        let mut state = self.search.lock().expect("search mutex poisoned");
        state.scope = scope;
        state.generation = state.generation.wrapping_add(1);
    }

    /// How far back the accounts in `searched` hold mail: the **narrowest** of their sync
    /// depths, since one three-month account makes the whole answer three months old at best.
    ///
    /// `None` when the scope searched no account at all: there is then no corpus to describe,
    /// and claiming "all mail" for a search that ran nowhere would be the opposite of the point.
    pub(crate) fn search_horizon<'a>(
        &self,
        searched: impl IntoIterator<Item = &'a AccountId>,
    ) -> Option<SearchHorizon> {
        searched
            .into_iter()
            .map(|account| self.effective_sync_depth(account.as_str()))
            .map(|depth| match depth {
                SyncDepth::AllTime => SearchHorizon::AllTime,
                SyncDepth::Months(months) => SearchHorizon::Months(months),
            })
            .reduce(narrowest)
    }

    /// Runs the search itself: every account the `scope` covers, searched in turn, each
    /// account's ranked candidate set resolved from the store and filtered to the scope, pooled
    /// into one unordered hit list.
    ///
    /// Split out of [`search_snapshot`](Self::search_snapshot) so the **read** side of the agent
    /// adapter (`query::search`) can reuse it. That matters more than it looks: which accounts
    /// and folders a search covers is rule 3 of `docs/search.md`, and a second implementation of
    /// it would be a second answer to "what does a search cover": the exact drift that contract
    /// exists to prevent. Ordering is deliberately **not** here; it lives once in
    /// `view_rows::build_search`, which both callers hand their hits to.
    pub(crate) async fn search_hits(
        &self,
        query: &str,
        scope: SearchScope,
        selected_account: Option<&AccountId>,
        selected_folder: Option<&str>,
        account_rows: &[AccountRow],
        mailboxes: &[(AccountId, Vec<Mailbox>)],
    ) -> Vec<AccountMessage> {
        let mut hits: Vec<AccountMessage> = Vec::new();
        for (id, filter) in searched(scope, selected_account, selected_folder, mailboxes) {
            let ranked = self
                .engine
                .search_mail(id, query, SEARCH_FETCH_LIMIT)
                .await
                .map(|results| results.hits)
                .unwrap_or_default();
            // Resolve the hit keys straight from the store, not the message cache: the cache
            // holds only the newest-N window, but a search legitimately matches older messages
            // outside it, so a windowed cache would silently drop those hits.
            let keys: Vec<ProviderKey> = ranked.into_iter().map(|hit| hit.key).collect();
            let resolved = self
                .engine
                .mail_by_keys(id, &keys)
                .await
                .unwrap_or_default();
            let owner = owner_email(account_rows, id.as_str());
            hits.extend(
                resolved
                    .into_iter()
                    .filter(|row| filter.keeps(row))
                    .map(|row| AccountMessage {
                        // **`in_scope` means "matched the query" here**, not "is in the folder
                        // on screen": a search is its own list. The threaded projection reads
                        // it twice, and both readings are the ones this search wants: a
                        // conversation is listed only if a message in it matched, and it sorts
                        // on its newest *matching* message, so a thread does not climb the
                        // results because of a reply that has nothing to do with the query.
                        outgoing: is_outgoing(&row, owner),
                        row: Arc::new(row),
                        in_scope: true,
                    }),
            );
        }
        hits
    }

    /// Adds the **non-matching** members of every conversation the hits touch, so a threaded
    /// search row carries its whole conversation rather than the fragment that matched.
    ///
    /// Each addition is marked out of scope (`in_scope: false`), which is what keeps the two
    /// rules of a threaded search true: a conversation nobody's query touched is never listed,
    /// and a thread is ordered by its newest match rather than by its newest message. Its
    /// members still render, and expand, and dedup against their cross-folder copies, exactly
    /// as they do in the mailbox list.
    ///
    /// One indexed read for every shown thread, never one per thread; the same read the mailbox
    /// list's completion makes.
    async fn complete_search_threads(
        &self,
        hits: &mut Vec<AccountMessage>,
        account_rows: &[AccountRow],
        mailboxes: &[(AccountId, Vec<Mailbox>)],
    ) {
        let threads: HashSet<&str> = hits
            .iter()
            .filter_map(|hit| hit.row.mail.thread_id.as_ref().map(ThreadId::as_str))
            .collect();
        if threads.is_empty() {
            return;
        }
        let present: HashSet<(&str, &str)> =
            hits.iter().map(|hit| (hit.account(), hit.key())).collect();
        let hidden = self.pending_hidden_keys();
        let accounts: Vec<AccountId> = mailboxes.iter().map(|(id, _)| id.clone()).collect();
        let started = Instant::now();
        let extra = self
            .engine
            .mail_on_threads(&accounts, threads.iter().copied())
            .await
            .unwrap_or_default();
        log::debug!(
            "search thread completion: {} thread(s) -> {} member(s) in {}ms",
            threads.len(),
            extra.len(),
            started.elapsed().as_millis(),
        );
        let added: Vec<AccountMessage> = extra
            .into_iter()
            .filter(|row| {
                !present.contains(&(row.account.as_str(), row.mail.key.as_str()))
                    && !hidden.contains(&(
                        row.account.as_str().to_owned(),
                        row.mail.key.as_str().to_owned(),
                    ))
            })
            .map(|row| AccountMessage {
                outgoing: is_outgoing(&row, owner_email(account_rows, row.account.as_str())),
                row: Arc::new(row),
                in_scope: false,
            })
            .collect();
        hits.extend(added);
    }

    /// Every account's folder list, in switcher order.
    pub(crate) async fn account_mailboxes(
        &self,
        account_rows: &[AccountRow],
    ) -> Vec<(AccountId, Vec<Mailbox>)> {
        let mut out = Vec::with_capacity(account_rows.len());
        for row in account_rows {
            let Ok(id) = AccountId::try_from(row.id.as_str()) else {
                continue;
            };
            let folders = self.engine.mailboxes(&id).await.unwrap_or_default();
            out.push((id, folders));
        }
        out
    }
}

/// The search the user is looking at: what they typed, and a counter that dates it.
///
/// One value behind one lock, because a rebuild has to read both and they have to agree: read
/// separately, a rebuild can capture the query a keystroke has just written while still holding
/// the generation from before it, and then discard a list that was current after all.
///
/// `query` is `None` when the list is not a search; a blank field clears rather than searching
/// for "" (`Intent::Search`). `scope` is which folders it covers. Both are session state, never
/// persisted, and they travel together because a rebuild needs both to mean one search.
///
/// The generation counts every change, a scope change included, and wraps: nothing reads it as a
/// quantity, only for equality with the value a rebuild started from.
#[derive(Clone, Default)]
pub(crate) struct SearchState {
    pub(crate) query: Option<String>,
    pub(crate) scope: SearchScope,
    pub(crate) generation: u64,
}

/// The accounts the active `scope` searches, each paired with the filter its hits must pass.
///
/// One list, two callers: the search itself iterates it, and the horizon is folded over the
/// accounts in it. Deriving the horizon from a second walk of `mailboxes` would be a second
/// answer to "which accounts does a search cover"; rule 4 of `docs/search.md`, and the drift
/// that contract exists to prevent.
pub(crate) fn searched<'a>(
    scope: SearchScope,
    selected_account: Option<&AccountId>,
    selected_folder: Option<&str>,
    mailboxes: &'a [(AccountId, Vec<Mailbox>)],
) -> Vec<(&'a AccountId, HitFilter)> {
    mailboxes
        .iter()
        .filter_map(|(id, folders)| {
            HitFilter::for_scope(scope, id, folders, selected_account, selected_folder)
                .map(|filter| (id, filter))
        })
        .collect()
}

/// The narrower of two horizons; any month count beats "all mail", and the smaller count wins.
fn narrowest(left: SearchHorizon, right: SearchHorizon) -> SearchHorizon {
    match (left, right) {
        (SearchHorizon::AllTime, other) | (other, SearchHorizon::AllTime) => other,
        (SearchHorizon::Months(a), SearchHorizon::Months(b)) => SearchHorizon::Months(a.min(b)),
    }
}

/// Which of one account's matched messages the active scope keeps.
pub(crate) enum HitFilter {
    /// Everything except the account's Trash (`None` when it has no trash folder, in which
    /// case nothing is excluded).
    ExceptTrash(Option<String>),
    /// Only the messages filed in this folder key.
    Only(String),
    /// Everything the account holds, Trash included; what "current folder" means when the
    /// account's own all-mail view is showing, and the only way to search trashed mail.
    Everything,
}

impl HitFilter {
    /// The filter for one account under `scope`, or `None` when the account is outside the
    /// scope entirely and should not be searched at all.
    pub(crate) fn for_scope(
        scope: SearchScope,
        account: &AccountId,
        folders: &[Mailbox],
        selected_account: Option<&AccountId>,
        selected_folder: Option<&str>,
    ) -> Option<Self> {
        match scope {
            SearchScope::AllFolders => {
                Some(Self::ExceptTrash(role_key(folders, &MailboxRole::Trash)))
            }
            SearchScope::CurrentFolder => match selected_account {
                // One account is open: only it contributes, narrowed to the selected folder;
                // or its whole mailbox when the account's all-mail view is showing.
                Some(selected) if selected == account => Some(
                    selected_folder
                        .map_or(Self::Everything, |folder| Self::Only(folder.to_owned())),
                ),
                Some(_) => None,
                // The unified view: "current folder" is the set of inboxes on screen. An
                // account with no Inbox role shows nothing there, so it searches nothing here.
                None => role_key(folders, &MailboxRole::Inbox).map(Self::Only),
            },
        }
    }

    /// Whether this filter keeps `row`.
    pub(crate) fn keeps(&self, row: &MailListRow) -> bool {
        let filed_in = |key: &str| row.mailboxes.iter().any(|mb| mb.as_str() == key);
        match self {
            Self::ExceptTrash(trash) => trash.as_deref().is_none_or(|key| !filed_in(key)),
            Self::Only(key) => filed_in(key),
            Self::Everything => true,
        }
    }
}

/// The provider key of the folder `folders` tags with `role`, or `None`; resolved from the
/// already-read folder list rather than the app's per-role key cache, which holds only the
/// inbox.
pub(crate) fn role_key(folders: &[Mailbox], role: &MailboxRole) -> Option<String> {
    folders
        .iter()
        .find(|mailbox| mailbox.role.as_ref() == Some(role))
        .map(|mailbox| mailbox.id.key().as_str().to_owned())
}
