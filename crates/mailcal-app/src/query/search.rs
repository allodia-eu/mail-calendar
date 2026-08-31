//! Searching mail as a query: the read side of "find the one about invoices".
//!
//! Split out of `query.rs` to stay under the 500-line limit.

use engine_api::{AccountId, Provider};
use mailcal_viewmodel::view;

use super::{MessagePage, list::page_from};
use crate::{App, SearchScope, snapshot_search::searched};

impl<P: Provider> App<P> {
    /// Searches mail and returns one page of hits, **newest first**.
    ///
    /// # The scope vocabulary is not reinvented here
    ///
    /// `docs/search.md` already decides what a search covers, and `HitFilter::for_scope` already
    /// implements it. This maps the query's two optional narrowings onto that same vocabulary
    /// rather than growing a second answer:
    ///
    /// | `account` | `folder` | Scope |
    /// |---|---|---|
    /// | `None` |; | every account, every folder **except Trash** (the product default) |
    /// | `Some` | `None` | that account's whole mailbox, Trash included |
    /// | `Some` | `Some` | that one folder |
    ///
    /// Trash staying out of the default and reachable by narrowing to it is rule 2 of that
    /// contract, and it holds here for free because the filter is shared.
    ///
    /// Ordering is newest-first, never by relevance; rule 1 of the same contract, and it holds
    /// for free too: the hits go through `view::search_results`, the identical projection the
    /// mailbox list uses. The engine's ranking still decides *which* hits arrive (it caps each
    /// account's candidate set), which is why a broad query can miss a recent match, that
    /// shortfall is `docs/search.md`'s, not a new one.
    ///
    /// **Nothing about the user's own search is touched**: this reads neither the active query
    /// nor the active scope, and writes neither. An assistant searching cannot narrow, widen or
    /// clear what the person is looking at.
    pub async fn query_search(
        &self,
        query: &str,
        account: Option<&AccountId>,
        folder: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> MessagePage {
        if query.trim().is_empty() {
            return MessagePage::default();
        }
        let account_rows = self.account_rows().await;
        let mailboxes = self.account_mailboxes(&account_rows).await;
        // No account named ⇒ everything but Trash; an account named ⇒ "the current folder",
        // which resolves to that one folder, or to the account's whole mailbox when no folder
        // is named. Both already have defined semantics in `HitFilter::for_scope`.
        let scope = if account.is_some() {
            SearchScope::CurrentFolder
        } else {
            SearchScope::AllFolders
        };
        let searched = searched(scope, account, folder, &mailboxes);
        let hits = self
            .search_hits(query, scope, account, folder, &mailboxes)
            .await;
        let total = hits.len();
        let snapshot = view::search_results(&hits, &[], Vec::new(), offset.saturating_add(limit));
        let mut page = page_from(snapshot, offset);
        // `search_results` reports the *shown* row count as its total; the honest total is how
        // many hits survived the scope filter, which is what a caller pages against.
        page.total = total;
        page.horizon = self.search_horizon(searched.into_iter().map(|(account, _)| account));
        page
    }
}
