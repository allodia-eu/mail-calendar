//! The folder-tree half of the inbound protocol ([`FolderIntent`]), which
//! [`Intent::Folders`](super::Intent::Folders) carries.
//!
//! A family of its own for the reason [`OutboxIntent`](super::OutboxIntent) is one, and because
//! `intent.rs` is at its 500-line limit. Every change here names its folder by a
//! [`FolderRef`], the account and the key together (`docs/folder-pane.md`, rule 14).

use engine_api::AccountId;

use crate::reference::{FolderRef, RowRef};

/// What the folder pane asks the runtime to do to an account's folders.
///
/// Each change goes through the engine's outbox, so one made offline is drawn in the pane at
/// once, marked pending, and reaches the server when the device is back online
/// (`docs/folder-pane.md`, "Changing the tree").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderIntent {
    /// Make a folder called `name` inside `parent`, or at the top of the account's tree.
    Create {
        /// The account to make it in.
        account: AccountId,
        /// The folder to make it inside, a key of `account`'s, or `None` for the top level.
        parent: Option<String>,
        /// The new folder's own name.
        name: String,
    },
    /// Give a folder a new name, where it is.
    Rename {
        /// The folder to rename.
        folder: FolderRef,
        /// The name it is to have.
        name: String,
    },
    /// Move a folder, and everything inside it, into another folder of the same account, or
    /// to the top of its tree. What a folder dropped on a folder or an account row asks for.
    Move {
        /// The folder to move.
        folder: FolderRef,
        /// The folder to move it into, a key of the same account's, or `None` for the top.
        parent: Option<String>,
    },
    /// Delete a folder: into Trash, with everything inside it, or for good when it is already
    /// in Trash. A client confirms first and words the question by `FolderRow::in_trash`.
    Delete(FolderRef),
    /// Move messages into a folder: what rows dropped on a folder ask for. A row of another
    /// account than the folder's is left where it is; mail moves only within its account.
    MoveMessages {
        /// The rows, messages or whole conversations, that were dropped.
        rows: Vec<RowRef>,
        /// The folder they were dropped on.
        folder: FolderRef,
    },
    /// Take the standing folder notice off the pane.
    DismissNotice,
}
