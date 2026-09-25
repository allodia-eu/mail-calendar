//! What the folder pane offers on a row, and where a folder or a message may be dropped.
//!
//! Pure decisions over the snapshot's rows, so they are tested without a display; the widgets in
//! `folder_menu`, `folder_dialogs` and `folder_drag` only read them. The core stamps every row with
//! what it allows (`docs/folder-pane.md`, rule 22), so nothing here decides eligibility of its own:
//! it only turns those flags into menu items, candidates and answers.

use std::{collections::HashSet, rc::Rc, sync::Arc};

use mailcal_bindings::{FolderIntent, FolderNameCheck, FolderRow, Intent, MailcalApp, SelectedRow};

use super::{AppModel, folder_names::folder_label};
use crate::l10n;

/// One entry of a pane row's context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FolderMenuItem {
    NewFolder,
    Rename,
    MoveTo,
    Delete,
}

impl FolderMenuItem {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NewFolder => l10n::folder_action_new(),
            Self::Rename => l10n::folder_action_rename(),
            Self::MoveTo => l10n::folder_action_move(),
            Self::Delete => l10n::folder_action_delete(),
        }
    }
}

/// The menu a folder row offers, in the order rule 23 gives. Empty for a pending row: every flag
/// the core stamps on one is off.
pub(crate) fn folder_menu(row: &FolderRow) -> Vec<FolderMenuItem> {
    let mut items = Vec::new();
    if row.accepts_folders {
        items.push(FolderMenuItem::NewFolder);
    }
    if row.editable {
        items.extend([
            FolderMenuItem::Rename,
            FolderMenuItem::MoveTo,
            FolderMenuItem::Delete,
        ]);
    }
    items
}

/// The menu an account row offers: New folder, where the account's tree can change at all.
pub(crate) fn account_menu(manages_folders: bool) -> Vec<FolderMenuItem> {
    if manages_folders {
        vec![FolderMenuItem::NewFolder]
    } else {
        Vec::new()
    }
}

/// `key` and every folder filed beneath it, by walking the rows' own parents.
pub(crate) fn subtree(folders: &[FolderRow], key: &str) -> HashSet<String> {
    let mut inside = HashSet::from([key.to_owned()]);
    // Each round adds the next level down; a tree can be no deeper than it has rows.
    for _ in 0..folders.len() {
        let before = inside.len();
        for folder in folders {
            if folder
                .parent
                .as_ref()
                .is_some_and(|parent| inside.contains(parent))
            {
                inside.insert(folder.key.clone());
            }
        }
        if inside.len() == before {
            break;
        }
    }
    inside
}

/// Each row's name with the folders it sits inside, `Clients / Acme`, in the user's words.
fn path_label(folders: &[FolderRow], row: &FolderRow) -> String {
    let mut parts = vec![folder_label(row.role.as_ref(), &row.name)];
    let mut parent = row.parent.as_deref();
    while let Some(key) = parent.filter(|_| parts.len() <= folders.len()) {
        let Some(ancestor) = folders.iter().find(|folder| folder.key == key) else {
            break;
        };
        parts.push(folder_label(ancestor.role.as_ref(), &ancestor.name));
        parent = ancestor.parent.as_deref();
    }
    parts.reverse();
    parts.join(" / ")
}

/// One destination Move to… offers: a folder to move into, or the top of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MoveCandidate {
    /// The folder to move into, or `None` for the top level.
    pub(crate) parent: Option<String>,
    pub(crate) label: String,
}

/// The destinations Move to… lists for `moving`: Top level first, then every folder that takes
/// folders, leaving out the one being moved and everything inside it (rule 24).
pub(crate) fn move_candidates(folders: &[FolderRow], moving: &str) -> Vec<MoveCandidate> {
    let excluded = subtree(folders, moving);
    let top = MoveCandidate {
        parent: None,
        label: l10n::folder_move_top_level().to_owned(),
    };
    std::iter::once(top)
        .chain(
            folders
                .iter()
                .filter(|folder| folder.accepts_folders && !excluded.contains(&folder.key))
                .map(|folder| MoveCandidate {
                    parent: Some(folder.key.clone()),
                    label: path_label(folders, folder),
                }),
        )
        .collect()
}

/// What is being dragged across the pane: always one account's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Dragged {
    /// A folder, with the folder it sits in now, so a drop back where it came from is refused.
    Folder { key: String, parent: Option<String> },
    /// A message or a conversation from the list. When it is one of the selected rows, the whole
    /// selection goes with it, as a row menu does (`docs/list-selection.md`, rule 12).
    Mail(SelectedRow),
}

/// The keys of the folders `key` sits inside, nearest first: what decides whether a folder
/// dropped on this row would land inside itself.
pub(crate) fn ancestors(folders: &[FolderRow], key: &str) -> Vec<String> {
    let mut chain = Vec::new();
    let mut at = folders.iter().find(|folder| folder.key == key);
    while let Some(parent) = at.and_then(|folder| folder.parent.clone()) {
        if chain.len() > folders.len() || chain.contains(&parent) {
            break;
        }
        at = folders.iter().find(|folder| folder.key == parent);
        chain.push(parent);
    }
    chain
}

/// Where a drag is released, as the row's own flags describe it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DropSpot {
    /// An account row: a folder dropped here moves to the top of that account's tree.
    Account { manages_folders: bool },
    /// A folder row.
    Folder {
        key: String,
        accepts_folders: bool,
        accepts_messages: bool,
        /// The folders it sits inside, from [`ancestors`].
        ancestors: Vec<String>,
    },
}

impl DropSpot {
    pub(crate) fn of(row: &FolderRow, folders: &[FolderRow]) -> Self {
        Self::Folder {
            key: row.key.clone(),
            accepts_folders: row.accepts_folders,
            accepts_messages: row.accepts_messages,
            ancestors: ancestors(folders, &row.key),
        }
    }
}

/// Whether `spot`, in `account`, takes `dragged` from `from`. Never across accounts, never onto a
/// pending row (whose flags are all off), and never a folder into itself, into a folder inside
/// it, or back where it already is.
pub(crate) fn accepts_drop(account: &str, spot: &DropSpot, from: &str, dragged: &Dragged) -> bool {
    if account != from {
        return false;
    }
    match (spot, dragged) {
        (DropSpot::Account { manages_folders }, Dragged::Folder { parent, .. }) => {
            *manages_folders && parent.is_some()
        }
        (DropSpot::Account { .. }, Dragged::Mail(_)) => false,
        (
            DropSpot::Folder {
                key: target,
                accepts_folders,
                ancestors,
                ..
            },
            Dragged::Folder { key, parent },
        ) => {
            *accepts_folders
                && target != key
                && parent.as_ref() != Some(target)
                && !ancestors.contains(key)
        }
        (
            DropSpot::Folder {
                accepts_messages, ..
            },
            Dragged::Mail(_),
        ) => *accepts_messages,
    }
}

/// The line the name dialog shows under its field, or `None` when the name can be used. Empty
/// says nothing: the disabled button already does, and a warning before the first keystroke
/// would be about a name nobody has typed.
pub(crate) fn name_problem(check: FolderNameCheck) -> Option<&'static str> {
    match check {
        FolderNameCheck::Valid | FolderNameCheck::Empty => None,
        FolderNameCheck::Surrounded => Some(l10n::folder_name_surrounded()),
        FolderNameCheck::Control => Some(l10n::folder_name_control()),
        FolderNameCheck::Separator => Some(l10n::folder_name_separator()),
        FolderNameCheck::Taken => Some(l10n::folder_name_taken()),
    }
}

/// The words of a delete confirmation: a move to Trash outside it, the end inside it (rule 26).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeleteCopy {
    pub(crate) title: String,
    pub(crate) message: &'static str,
    pub(crate) confirm: &'static str,
    pub(crate) destructive: bool,
}

pub(crate) fn delete_copy(name: &str, in_trash: bool) -> DeleteCopy {
    if in_trash {
        DeleteCopy {
            title: l10n::folder_delete_permanent_title(name),
            message: l10n::folder_delete_permanent_message(),
            confirm: l10n::action_delete_permanently(),
            destructive: true,
        }
    } else {
        DeleteCopy {
            title: l10n::folder_delete_title(name),
            message: l10n::folder_delete_message(),
            confirm: l10n::action_move_to_trash(),
            destructive: false,
        }
    }
}

/// A name the dialog asks about: which account, inside which folder, and which folder is being
/// renamed (it may keep its own name).
pub(crate) struct NameQuery<'a> {
    pub(crate) account: &'a str,
    pub(crate) parent: Option<&'a str>,
    pub(crate) name: &'a str,
    pub(crate) renaming: Option<&'a str>,
}

/// Answers a name dialog as the user types.
pub(crate) type NameCheck = Rc<dyn Fn(&NameQuery<'_>) -> FolderNameCheck>;

/// The core's answer where there is a core; without one (a window drawn before boot) nothing can be
/// created anyway, so every name is refused.
pub(crate) fn name_check(app: Option<Arc<MailcalApp>>) -> NameCheck {
    Rc::new(move |query| {
        app.as_ref().map_or(FolderNameCheck::Empty, |app| {
            app.check_folder_name(
                query.account.to_owned(),
                query.parent.map(str::to_owned),
                query.name.to_owned(),
                query.renaming.map(str::to_owned),
            )
        })
    })
}

/// What the pane asks of the model: a change to send, or mail dropped on a folder.
#[derive(Debug)]
pub(crate) enum FolderInput {
    Change(FolderIntent),
    DropMail {
        row: SelectedRow,
        account: String,
        key: String,
    },
}

impl AppModel {
    pub(super) fn folder_input(&mut self, input: FolderInput) {
        let intent = match input {
            FolderInput::Change(intent) => intent,
            FolderInput::DropMail { row, account, key } => {
                // The dragged row stands for the selection when it is one of the selected rows,
                // and the selection leaves with it: its rows are moving out of the list.
                let rows = if self.selection.selected_rows().contains(&row) {
                    let rows = self.selection.selected_rows();
                    self.selection.clear();
                    rows
                } else {
                    vec![row]
                };
                FolderIntent::MoveMessages { rows, account, key }
            }
        };
        self.dispatch(Intent::Folders { intent });
    }
}

#[cfg(test)]
#[path = "folder_actions_tests.rs"]
mod tests;
