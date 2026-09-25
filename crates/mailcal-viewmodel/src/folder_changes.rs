//! Folder changes the server has not confirmed yet, what each folder row lets the user do,
//! and what the pane says when a change is refused.
//!
//! A change to the folder tree goes through the engine's outbox and, offline, can wait there
//! for hours. The store holds only what the server has confirmed, so the pane draws the queued
//! changes over it ([`with_folder_changes`]) and marks each row they touch as
//! [`pending`](FolderRow::pending): the user sees the folder they made, and nothing lets them
//! build on a key that may still move.

use std::collections::{BTreeMap, BTreeSet};

use engine_api::{Mailbox, MailboxChange, MailboxId, MailboxNameError, validate_mailbox_name};

use crate::{FolderRole, FolderRow};

/// One folder change still in the account's outbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedFolderChange {
    /// The outbox op carrying it.
    pub op: u64,
    /// What the user asked for.
    pub change: MailboxChange,
}

/// The key a folder that is still being made is drawn under, until the server gives it one.
#[must_use]
pub fn pending_folder_key(op: u64) -> String {
    format!("pending-folder:{op}")
}

/// The stored folders with every queued change drawn over them, and the keys those changes
/// touch.
///
/// A queued create becomes a folder under [`pending_folder_key`]; a rename, move or trash puts
/// the folder where the user put it; a permanent delete takes the folder and everything inside
/// it off the list. A change naming a folder the list does not hold draws nothing: the engine
/// will refuse it when it runs.
#[must_use]
pub fn with_folder_changes(
    stored: &[Mailbox],
    queued: &[QueuedFolderChange],
) -> (Vec<Mailbox>, BTreeSet<String>) {
    let mut folders = stored.to_vec();
    let mut pending = BTreeSet::new();
    for QueuedFolderChange { op, change } in queued {
        match change {
            MailboxChange::Create { name, parent } => {
                let key = pending_folder_key(*op);
                if let Ok(id) = MailboxId::try_from(key.as_str()) {
                    let mut made = Mailbox::new(id, name.clone());
                    made.parent.clone_from(parent);
                    folders.push(made);
                    pending.insert(key);
                }
            }
            MailboxChange::Update {
                target,
                name,
                parent,
                ..
            } => {
                if let Some(folder) = folders.iter_mut().find(|m| &m.id == target) {
                    folder.name.clone_from(name);
                    folder.parent.clone_from(parent);
                    pending.insert(target.as_str().to_owned());
                }
            }
            MailboxChange::Trash { target, trash, .. } => {
                if let Some(folder) = folders.iter_mut().find(|m| &m.id == target) {
                    folder.parent = Some(trash.clone());
                    pending.insert(target.as_str().to_owned());
                }
            }
            MailboxChange::Delete { target } => {
                let gone = subtree(&folders, target);
                folders.retain(|m| !gone.contains(m.id.as_str()));
            }
        }
    }
    (folders, pending)
}

/// `root` and every folder beneath it, by key.
fn subtree(folders: &[Mailbox], root: &MailboxId) -> BTreeSet<String> {
    let mut gone = BTreeSet::from([root.as_str().to_owned()]);
    // Each round adds the next level down; a list can be no deeper than it is long.
    for _ in 0..folders.len() {
        let before = gone.len();
        for folder in folders {
            if folder
                .parent
                .as_ref()
                .is_some_and(|parent| gone.contains(parent.as_str()))
            {
                gone.insert(folder.id.as_str().to_owned());
            }
        }
        if gone.len() == before {
            break;
        }
    }
    gone
}

/// Fills in what each row lets the user do, walking the rows in the depth-first order
/// [`sorted_folder_rows`](crate::sorted_folder_rows) gives them, so every folder is stamped
/// after the one it is drawn inside.
///
/// `manages_folders` is the account's: whether its provider can change the tree at all.
/// Dropping mail needs no such capability; every provider that syncs mail can move it.
pub fn stamp_folder_actions(
    rows: &mut [FolderRow],
    manages_folders: bool,
    pending: &BTreeSet<String>,
) {
    // Per key already stamped: whether it is pending, and whether a folder inside it is in
    // Trash (it is Trash, or sits in it).
    let mut seen: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for row in rows.iter_mut() {
        let (parent_pending, parent_in_trash) = row
            .parent
            .as_ref()
            .and_then(|parent| seen.get(parent))
            .copied()
            .unwrap_or_default();
        row.pending = parent_pending || pending.contains(&row.key);
        row.in_trash = parent_in_trash;
        let own_trash = row.role == Some(FolderRole::Trash);
        seen.insert(row.key.clone(), (row.pending, row.in_trash || own_trash));

        row.editable = manages_folders && row.role.is_none() && !row.pending;
        row.accepts_folders = manages_folders
            && !row.pending
            && !row.in_trash
            && !matches!(
                row.role,
                Some(FolderRole::Trash | FolderRole::Junk | FolderRole::Drafts | FolderRole::Other)
            );
        // Junk takes mail through a report, never a move (`docs/reporting.md`), and Drafts and
        // the virtual folders are not somewhere mail is filed.
        row.accepts_messages = !row.pending
            && !matches!(
                row.role,
                Some(FolderRole::Junk | FolderRole::Drafts | FolderRole::Other)
            );
    }
}

/// Whether a name can be given to a folder, answered while the user is still typing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderNameCheck {
    /// The name can be used.
    Valid,
    /// The name is empty.
    Empty,
    /// The name starts or ends with a space.
    Surrounded,
    /// The name carries a control character.
    Control,
    /// The name carries `/`, which most servers read as a level of the tree.
    Separator,
    /// A folder beside it already has that name, compared without case.
    Taken,
}

/// Checks `name` for a folder inside `parent` (`None`: the top of the account's tree),
/// against the rules every transport shares and the folders already there. `renaming` is the
/// folder being renamed, which may keep its own name.
///
/// Compared without case because a server that folds case refuses the collision, and two
/// folders a user cannot tell apart are a mistake either way.
#[must_use]
pub fn check_folder_name(
    folders: &[Mailbox],
    parent: Option<&str>,
    name: &str,
    renaming: Option<&str>,
) -> FolderNameCheck {
    if let Err(err) = validate_mailbox_name(name) {
        return match err {
            MailboxNameError::Empty => FolderNameCheck::Empty,
            MailboxNameError::Surrounded => FolderNameCheck::Surrounded,
            MailboxNameError::Control => FolderNameCheck::Control,
            MailboxNameError::Separator => FolderNameCheck::Separator,
        };
    }
    let wanted = name.to_lowercase();
    let taken = folders.iter().any(|folder| {
        folder.parent.as_ref().map(MailboxId::as_str) == parent
            && Some(folder.id.as_str()) != renaming
            && folder.name.to_lowercase() == wanted
    });
    if taken {
        FolderNameCheck::Taken
    } else {
        FolderNameCheck::Valid
    }
}

/// A folder change the server refused, standing on the pane until the user dismisses it or
/// makes another change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderNotice {
    /// The account whose folder it was.
    pub account: String,
    /// The folder's name as the user last saw it, or the name they asked for on a create.
    pub folder: String,
    /// What the user asked for.
    pub action: FolderAction,
    /// Why it did not happen.
    pub problem: FolderProblem,
}

/// A change to the folder tree, as the user thinks of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderAction {
    /// Making a folder.
    Create,
    /// Renaming one.
    Rename,
    /// Moving one inside another, or to the top.
    Move,
    /// Deleting one: into Trash, or for good from inside it.
    Delete,
}

/// Why a folder change did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderProblem {
    /// The folder was changed, moved or removed elsewhere since this device last saw it, so
    /// the change was not applied over it.
    ChangedElsewhere,
    /// The server refused it.
    Refused,
}

#[cfg(test)]
#[path = "folder_changes_tests.rs"]
mod tests;
