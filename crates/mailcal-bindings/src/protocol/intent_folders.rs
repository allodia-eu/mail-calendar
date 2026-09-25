//! The folder-tree half of the FFI intent surface, nested behind one `Intent::Folders` variant
//! for the reason `OutboxIntent` is: [`Intent`](super::Intent) is at its 500-line limit. It
//! mirrors the core's own `FolderIntent`.

use uniffi::Enum;

use super::SelectedRow;

/// What the folder pane asks the app to do to an account's folders
/// (`docs/folder-pane.md`, "Changing the tree").
///
/// Every folder is named by its account **and** its key, as `Intent::SelectFolder` names one
/// (rule 14). Offer a change only where the row allows it (`FolderRow::editable`,
/// `accepts_folders`, `accepts_messages`, `AccountFolderRow::manages_folders`); the app ignores
/// one the row did not offer.
#[derive(Debug, Clone, PartialEq, Eq, Enum)]
pub enum FolderIntent {
    /// Make a folder called `name` inside `parent`, or at the top of the account's tree.
    /// Check the name first with `MailcalApp::check_folder_name`.
    Create {
        /// The account to make it in.
        account: String,
        /// The key of the folder to make it inside, or `None` for the top level.
        parent: Option<String>,
        /// The new folder's own name.
        name: String,
    },
    /// Give a folder a new name, where it is.
    Rename {
        /// The folder's account.
        account: String,
        /// The folder's key.
        key: String,
        /// The name it is to have.
        name: String,
    },
    /// Move a folder, with everything inside it, into another folder of the same account or
    /// to the top of its tree: a folder dropped on a folder row, or on its account's row.
    Move {
        /// The folder's account.
        account: String,
        /// The folder's key.
        key: String,
        /// The key of the folder to move it into, or `None` for the top level.
        parent: Option<String>,
    },
    /// Delete a folder: into Trash with everything inside it, or for good when
    /// `FolderRow::in_trash`. Confirm first, and word the question by `in_trash`.
    Delete {
        /// The folder's account.
        account: String,
        /// The folder's key.
        key: String,
    },
    /// Move messages into a folder: rows dropped on a folder row. Rows of another account
    /// than the folder's stay where they are.
    MoveMessages {
        /// The dropped rows, messages or whole conversations.
        rows: Vec<SelectedRow>,
        /// The folder's account.
        account: String,
        /// The folder's key.
        key: String,
    },
    /// Take the standing folder notice off the pane.
    DismissNotice,
}
