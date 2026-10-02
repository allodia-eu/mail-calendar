//! Move to folder… on a message row (`docs/folder-pane.md`, rule 24): which rows it moves, and
//! the folders it lists.
//!
//! The list keeps a row's widget while its rendering is unchanged, so a row outlives the selection
//! and the folder tree it was built against. Its menu therefore decides when it opens, from the
//! [`MoveContext`] every render leaves behind. The decision reads only the flags the core stamps
//! on each folder (rule 22), and is tested without a display.

use std::{cell::RefCell, collections::HashSet, rc::Rc};

use mailcal_bindings::{AccountFolderRow, MailboxListSnapshot, SelectedRow};

use super::{
    AppModel,
    folder_picker::{PickerFolder, PickerRow, picker_folders, picker_rows},
};

/// What a row's menu needs to know about the rest of the list when it opens.
#[derive(Clone, Debug, Default)]
pub(crate) struct MoveContext {
    /// The selected rows, which a menu opened on one of them acts on
    /// (`docs/list-selection.md`, rule 12).
    selected: Vec<SelectedRow>,
    /// Each account's tree, by id.
    trees: Vec<(String, AccountTree)>,
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
        let trees = snapshot
            .account_folders
            .iter()
            .map(|account| (account.account_id.clone(), AccountTree::of(account)))
            .collect();
        Self {
            selected,
            trees,
            showing,
        }
    }
}

/// One account's folders as a picker draws them, and which of them take mail.
#[derive(Clone, Debug, Default)]
struct AccountTree {
    folders: Vec<PickerFolder>,
    takes_mail: HashSet<String>,
}

impl AccountTree {
    fn of(account: &AccountFolderRow) -> Self {
        Self {
            folders: picker_folders(&account.folders),
            takes_mail: account
                .folders
                .iter()
                .filter(|folder| folder.accepts_messages)
                .map(|folder| folder.key.clone())
                .collect(),
        }
    }
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

/// Where the rows a menu acts on may go: always one account's folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MessageMove {
    pub(crate) account: String,
    /// The account's tree with no Top level: each folder that takes mail enabled, and a folder
    /// above one drawn disabled to keep it in place.
    pub(crate) choices: Vec<PickerRow>,
}

/// What Move to folder… offers on `row`'s menu, or `None` when it is not offered: the rows span
/// accounts, or no folder is left to choose. The destinations are the folders of the rows'
/// account that take mail, leaving out the one the list is showing.
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
    let tree = context
        .trees
        .iter()
        .find(|(id, _)| id == account)
        .map(|(_, tree)| tree)?;
    let showing = context
        .showing
        .as_ref()
        .filter(|(showing, _)| showing == account)
        .map(|(_, key)| key.as_str());
    let destinations: HashSet<String> = tree
        .takes_mail
        .iter()
        .filter(|key| Some(key.as_str()) != showing)
        .cloned()
        .collect();
    let choices = picker_rows(&tree.folders, &destinations, None);
    choices
        .iter()
        .any(|choice| choice.enabled)
        .then(|| MessageMove {
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
