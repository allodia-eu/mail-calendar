//! The menu a mailbox-list row opens (`docs/row-actions.md`), answered from the published
//! snapshot.
//!
//! A read, not an [`Intent`](crate::Intent): opening a menu changes nothing, so it is a direct
//! call a client makes when the menu opens, and the dispatch → snapshot loop never sees it. The
//! decisions are the view-model's ([`mailcal_viewmodel::row_menu`]); this layer supplies the two
//! things only the app knows, which folders each account's moves land in and which kind of
//! device this is.

use engine_api::{AccountId, Provider};
use mailcal_viewmodel::{AccountPlaces, AccountRow, MenuMessage, MenuRow, RowMenu};

use super::folders::account_places;
use crate::{
    App,
    form_factor::FormFactor,
    reference::{MessageRef, RowRef, ThreadRef},
};

impl<P: Provider> App<P> {
    /// The menu over `rows`: one row's own, or the batch over several
    /// (`docs/list-selection.md`, rule 12). Rows the list no longer shows are left out.
    #[must_use]
    pub fn row_menu(&self, rows: &[RowRef]) -> RowMenu {
        let rows: Vec<MenuRow> = rows.iter().map(menu_row).collect();
        let desktop = FormFactor::current() == FormFactor::Desktop;
        // Read in place: a client asks as the menu opens, on its UI thread, and a copy of the
        // whole list to answer about one row is a cost paid on every right-click.
        self.mailbox_list
            .read(|snapshot| mailcal_viewmodel::row_menu(snapshot, &rows, desktop))
    }

    /// The message the reading pane opens once `removed` leaves the list: the next one down, else
    /// the one above, opened as clicking its row opens it, so a draft resumes in a composer
    /// (`docs/reading-actions.md`, "Where the pane goes next"). `expanded` names the conversations
    /// the list shows expanded. Asked before the archive or delete is dispatched, while `removed`
    /// is still listed.
    #[must_use]
    pub fn stop_after_removing(
        &self,
        removed: &MessageRef,
        expanded: &[ThreadRef],
    ) -> Option<MenuMessage> {
        self.mailbox_list.read(|snapshot| {
            mailcal_viewmodel::stop_after_removing(
                snapshot,
                removed.account.as_str(),
                removed.key.as_str(),
                |thread| {
                    expanded.iter().any(|open| {
                        open.account.as_str() == thread.account
                            && open.thread_id == thread.thread_id
                    })
                },
            )
        })
    }

    /// Every account's [`AccountPlaces`], for the snapshot the menu is answered from.
    pub(crate) async fn all_account_places(
        &self,
        account_rows: &[AccountRow],
    ) -> Vec<AccountPlaces> {
        let mut places = Vec::with_capacity(account_rows.len());
        for row in account_rows {
            let Ok(account) = AccountId::try_from(row.id.as_str()) else {
                continue;
            };
            let mailboxes = self.engine.mailboxes(&account).await.unwrap_or_default();
            places.push(account_places(&account, &mailboxes));
        }
        places
    }
}

fn menu_row(row: &RowRef) -> MenuRow {
    match row {
        RowRef::Message(message) => MenuRow::Message {
            account: message.account.as_str().to_owned(),
            key: message.key.as_str().to_owned(),
        },
        RowRef::Thread(thread) => MenuRow::Thread {
            account: thread.account.as_str().to_owned(),
            thread_id: thread.thread_id.clone(),
        },
    }
}

#[cfg(test)]
#[path = "row_menu_tests.rs"]
mod tests;
