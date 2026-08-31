//! Listing one folder as a page of rows: the read side of "what's in my inbox?".
//!
//! Split out of `query.rs` (which holds the types and the per-message read) to stay under the
//! 500-line limit.

use std::sync::Arc;

use engine_api::{AccountId, MailListRow, Provider};
use mailcal_viewmodel::{AccountMessage, ViewMode, view};

use super::{MAX_QUERY_WINDOW, MessagePage};
use crate::{App, MIN_LOAD_WINDOW};

impl<P: Provider> App<P> {
    /// One page of `folder`'s messages on `account`, newest first. `folder` is a provider key;
    /// `None` lists the account's whole mailbox. `unread_only` narrows to unread mail, and is
    /// reflected in [`MessagePage::total`] so a caller can say "12 unread" honestly.
    ///
    /// # Paging is a window, not a cursor
    ///
    /// The page is cut from the account's **newest-N** slice, read straight from the store. Two
    /// consequences a caller must be told about rather than left to discover:
    ///
    /// * A folder whose mail is all older than that slice is not reachable by raising `offset`.
    ///   [`MessagePage::windowed`] is set when that is possible, so the answer can be "older mail
    ///   exists I cannot reach" instead of "that folder is empty".
    /// * The read **bypasses the shared list cache** the UI projects from. That is deliberate: the
    ///   cache holds the depth it was asked for, so an agent paging to offset 4 000 would leave the
    ///   UI's cache four thousand rows deep and slow every later snapshot rebuild. A query must not
    ///   make the user's app worse.
    pub async fn query_folder_page(
        &self,
        account: &AccountId,
        folder: Option<&str>,
        unread_only: bool,
        offset: usize,
        limit: usize,
    ) -> MessagePage {
        let requested = offset.saturating_add(limit);
        let window = requested.clamp(MIN_LOAD_WINDOW, MAX_QUERY_WINDOW);
        let base: Vec<MailListRow> = self
            .engine
            .mail_window(std::slice::from_ref(account), window)
            .await
            .unwrap_or_default();
        // The window was filled to the brim, so the account holds mail beyond it.
        let windowed = base.len() >= window;
        // Honour the optimistic hides, so a message the agent just archived does not reappear in
        // the very next listing while the move is still settling server-side.
        let hidden = self.pending_hidden_keys();
        let items: Vec<AccountMessage> = base
            .into_iter()
            .filter(|row| {
                !hidden.contains(&(
                    row.account.as_str().to_owned(),
                    row.mail.key.as_str().to_owned(),
                ))
            })
            .map(|row| {
                let keep = in_folder(&row, folder) && (!unread_only || row.mail.flags.is_unread());
                AccountMessage {
                    row: Arc::new(row),
                    // `in_scope` is the flat projection's filter; the caller's folder + unread
                    // narrowing rides on it so `total` counts exactly what matched.
                    in_scope: keep,
                    outgoing: false,
                }
            })
            .collect();
        // The same projection the mailbox list uses: it sorts the WHOLE in-scope set before
        // taking the window, so asking for `offset + limit` and slicing gives byte-identical
        // ordering to the UI's own first page. `total` is the full in-scope count.
        let snapshot = view::build(
            &items,
            &[],
            &[],
            Vec::new(),
            Some(account.as_str()),
            folder,
            ViewMode::Flat,
            requested,
        );
        MessagePage {
            windowed,
            horizon: self.search_horizon([account]),
            ..page_from(snapshot, offset)
        }
    }
}

/// Whether `row` is filed in `folder` (`None` = the account's whole mailbox).
fn in_folder(row: &MailListRow, folder: Option<&str>) -> bool {
    folder.is_none_or(|key| row.mailboxes.iter().any(|id| id.as_str() == key))
}

/// Slices a flat snapshot's rows from `offset`, keeping its `total`. Shared with the search
/// query so both page the same way.
pub(super) fn page_from(
    snapshot: mailcal_viewmodel::MailboxListSnapshot,
    offset: usize,
) -> MessagePage {
    let total = snapshot.total;
    let rows = snapshot
        .rows
        .into_iter()
        .skip(offset)
        .filter_map(|row| match row {
            mailcal_viewmodel::SnapshotRow::Flat(row) => Some(row),
            // Unreachable: a query always projects `ViewMode::Flat`. Dropping rather than
            // panicking keeps a future threaded projection from taking the process down.
            mailcal_viewmodel::SnapshotRow::Thread(_) => None,
        })
        .collect();
    MessagePage {
        rows,
        total,
        offset,
        windowed: false,
        horizon: None,
    }
}
