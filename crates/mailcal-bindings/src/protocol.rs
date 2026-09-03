//! The *in* half of the unidirectional loop: the [`Intent`]s a host dispatches (which live in
//! `intent` and are re-exported here) and the small enums they carry. The *out* half, the
//! surfaces a host observes and the callback that signals them, is `protocol_surface`. Both
//! splits exist to keep each file under the 500-line limit. These derive the UniFFI scaffolding,
//! so the generated Swift/Kotlin see them, and `lib.rs` re-exports them at the crate root.

// The intent enum in its own file, on the same 500-line grounds and in the same shape
// `mailcal-app` splits its own protocol module; `lib.rs` re-exports both halves, so a host
// sees no split at all.
mod intent;

pub use intent::Intent;

/// Which folders an active search covers: the host's scope filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SearchScope {
    /// Every account, every folder, except each account's Trash. The default.
    AllFolders,
    /// Only what the mailbox list was showing when the search started: the selected folder,
    /// or (in the unified view) every account's Inbox.
    CurrentFolder,
}

/// One selected mailbox-list row: a message (flat mode) or a whole conversation (threaded mode).
///
/// A client builds these straight from the rows it has highlighted; the core expands a
/// conversation into its messages itself, since the members come from the store's thread index
/// rather than from the snapshot (`docs/list-selection.md`).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SelectedRow {
    /// One message: a flat row, or one message of an expanded conversation.
    Message {
        /// The id of the account that owns the message (the row's `account`).
        account: String,
        /// The message's provider key.
        key: String,
    },
    /// One conversation: a threaded row, standing for every message on the thread.
    Thread {
        /// The id of the account that owns the conversation (the row's `account`).
        account: String,
        /// The thread's id (the row's `thread_id`).
        thread_id: String,
    },
}

/// What one action does to every selected row: the buttons a selection bar offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum BulkAction {
    /// Mark every selected message read.
    MarkRead,
    /// Mark every selected message unread.
    MarkUnread,
    /// Flag every selected message.
    Flag,
    /// Unflag every selected message.
    Unflag,
    /// Move every selected message to its account's Archive folder.
    Archive,
    /// Move every selected message to its account's Trash folder (recoverable).
    Delete,
    /// **Permanently** delete every selected message (irreversible: not a Trash move).
    PermanentlyDelete,
}

/// The answer a user can give to an invitation.
///
/// Three values because three is all there are: "no answer yet" is the *absence* of one, and
/// delegating is a different act this release does not offer. A client shows exactly these
/// buttons.
#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum InvitationResponse {
    /// Yes.
    Accept,
    /// Maybe.
    Tentative,
    /// No: the meeting then leaves the calendar (`docs/calendar.md`), reachable again from
    /// this card.
    Decline,
}
