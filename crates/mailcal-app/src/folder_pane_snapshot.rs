//! What the folder pane shows beside the message list: each account's folder tree, its
//! expansion at publish time, and the Outbox.
//!
//! Split from [`snapshot`](crate::snapshot), which assembles the list itself. These three
//! are the pane's own, and the pane is drawn in every view mode (`docs/folder-pane.md`,
//! rule 1), so they run on every rebuild regardless of what the list is showing.

use engine_api::{AccountId, Provider};
use mailcal_viewmodel::{
    AccountFolderRow, AccountRow, MailboxListSnapshot, QueuedRow, queued_rows,
};

use crate::App;

impl<P: Provider> App<P> {
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
    pub(crate) fn restamp_expansion(&self, snapshot: &mut MailboxListSnapshot) {
        for row in &mut snapshot.accounts {
            row.expanded = self.account_expanded(&row.id);
        }
        // The All Accounts group is one more tree in the same pane, and the projection never
        // sets it, so this is also where it is filled in at all.
        snapshot.unified_expanded = self.unified_expanded();
    }

    /// Fetches every account's sorted folder list in `account_rows` order: for the
    /// navigation drawer, which shows all accounts simultaneously.
    pub(crate) async fn all_account_folders(
        &self,
        account_rows: &[AccountRow],
    ) -> Vec<AccountFolderRow> {
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

    /// Every account's queued sends, oldest first within each account.
    ///
    /// One store read per account, on the same pass that reads their folders, because the
    /// pane's Outbox row carries a count and a count nobody refreshed is worse than none.
    /// Accounts are visited in `account_rows` order, so the list is stable between rebuilds
    /// and a row does not jump while the user is reaching for it.
    ///
    /// A failure to read one account's queue yields nothing for that account rather than
    /// failing the rebuild: the rest of the pane is still true, and the next rebuild tries
    /// again.
    pub(crate) async fn all_queued_sends(&self, account_rows: &[AccountRow]) -> Vec<QueuedRow> {
        let mut out = Vec::new();
        for row in account_rows {
            let Ok(id) = AccountId::try_from(row.id.as_str()) else {
                continue;
            };
            let queued = self.engine.outbox(&id).await.unwrap_or_default();
            out.extend(queued_rows(&row.id, &queued));
        }
        out
    }
}
