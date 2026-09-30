//! Move to folder… on a message row (`docs/folder-pane.md`, rule 24): which rows it moves, and
//! the folders it lists.
//!
//! The list keeps a row's widget while its rendering is unchanged, so a row outlives the selection
//! and the folder tree it was built against. Its menu therefore decides when it opens, from the
//! [`MoveContext`] every render leaves behind. The decision reads only the flags the core stamps
//! on each folder (rule 22), and is tested without a display.

use std::{cell::RefCell, rc::Rc};

use mailcal_bindings::{AccountFolderRow, MailboxListSnapshot, SelectedRow};

use super::{AppModel, folder_actions::path_label};

/// What a row's menu needs to know about the rest of the list when it opens.
#[derive(Clone, Debug, Default)]
pub(crate) struct MoveContext {
    /// The selected rows, which a menu opened on one of them acts on
    /// (`docs/list-selection.md`, rule 12).
    selected: Vec<SelectedRow>,
    /// Each account's folders that take mail, by id.
    takes_mail: Vec<(String, Vec<FolderChoice>)>,
    /// The account and folder the list is showing, when it is one folder.
    showing: Option<(String, String)>,
}

impl MoveContext {
    pub(crate) fn new(
        snapshot: &MailboxListSnapshot,
        selected: Vec<SelectedRow>,
        searching: bool,
    ) -> Self {
        let showing = match (&snapshot.selected_account, &snapshot.selected) {
            (Some(account), Some(folder)) if !searching => Some((account.clone(), folder.clone())),
            _ => None,
        };
        let takes_mail = snapshot
            .account_folders
            .iter()
            .map(|account| (account.account_id.clone(), takes_mail(account)))
            .collect();
        Self {
            selected,
            takes_mail,
            showing,
        }
    }
}

fn takes_mail(account: &AccountFolderRow) -> Vec<FolderChoice> {
    account
        .folders
        .iter()
        .filter(|folder| folder.accepts_messages)
        .map(|folder| FolderChoice {
            key: folder.key.clone(),
            label: path_label(&account.folders, folder),
        })
        .collect()
}

/// The context the list's rows share, replaced on every render.
pub(crate) type SharedMoveContext = Rc<RefCell<MoveContext>>;

impl AppModel {
    pub(super) fn move_context(&self) -> MoveContext {
        MoveContext::new(
            &self.snapshot,
            self.selection.selected_rows(),
            self.search.is_active(),
        )
    }
}

/// One folder Move to folder… lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FolderChoice {
    pub(crate) key: String,
    /// The folder's path, `Clients / Acme`, in the user's words.
    pub(crate) label: String,
}

/// Where the rows a menu acts on may go: always one account's folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageMove {
    pub(crate) account: String,
    pub(crate) choices: Vec<FolderChoice>,
}

/// What Move to folder… offers on `row`'s menu, or `None` when it is not offered: the rows span
/// accounts, or no folder is left to list. The folders are those of the rows' account that take
/// mail, leaving out the one the list is showing.
pub(crate) fn plan(context: &MoveContext, row: &SelectedRow) -> Option<MessageMove> {
    let rows = if context.selected.contains(row) {
        context.selected.as_slice()
    } else {
        std::slice::from_ref(row)
    };
    let account = account_of(rows.first()?);
    if rows.iter().any(|row| account_of(row) != account) {
        return None;
    }
    let folders = context
        .takes_mail
        .iter()
        .find(|(id, _)| id == account)
        .map_or(&[][..], |(_, folders)| folders.as_slice());
    let showing = context
        .showing
        .as_ref()
        .filter(|(showing, _)| showing == account)
        .map(|(_, key)| key.as_str());
    let choices: Vec<FolderChoice> = folders
        .iter()
        .filter(|folder| Some(folder.key.as_str()) != showing)
        .cloned()
        .collect();
    (!choices.is_empty()).then(|| MessageMove {
        account: account.to_owned(),
        choices,
    })
}

fn account_of(row: &SelectedRow) -> &str {
    match row {
        SelectedRow::Message { account, .. } | SelectedRow::Thread { account, .. } => account,
    }
}

#[cfg(test)]
#[path = "message_move_tests.rs"]
mod tests;
