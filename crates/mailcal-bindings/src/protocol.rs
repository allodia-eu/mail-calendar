//! The FFI protocol types of the unidirectional loop: the [`Surface`]s a host observes,
//! the [`Intent`]s it dispatches, and the [`Observer`] callback it implements. Split out
//! of `lib.rs` to keep it under the 500-line limit; these derive the UniFFI scaffolding
//! (so the generated Swift/Kotlin see them) and `lib.rs` re-exports them at the crate root.

// The intent enum in its own file, on the same 500-line grounds and in the same shape
// `mailcal-app` splits its own protocol module; `lib.rs` re-exports both halves, so a host
// sees no split at all.
mod intent;

pub use intent::Intent;

/// A surface a host observes and pulls a snapshot for.
#[derive(uniffi::Enum)]
pub enum Surface {
    /// The mailbox/message list.
    MailboxList,
    /// The calendar agenda.
    Calendar,
    /// The settings surface: the active display timezone and any pending change.
    Settings,
    /// The reading view: the open message's fetched, sanitised body.
    Reading,
    /// The outgoing-send status; drives the composer's "sending…" → "sent" hint.
    Sending,
    /// Background mail-download progress; drives a "downloading Y of X" bar (pulled via
    /// `MailcalApp::sync_progress`).
    SyncProgress,
    /// Connectivity: the device-offline flag and per-account outage list (pulled via
    /// `MailcalApp::connectivity`); drives the offline banner and per-account warning badges.
    Connectivity,
    /// Calendar write status: the outcome of the most recent create/edit/delete (pulled via
    /// `MailcalApp::calendar_write_status`); drives a small in-calendar spinner and warning.
    CalendarStatus,
    /// The contacts list: the unified people snapshot (pulled via `MailcalApp::contact_list`).
    Contacts,
    /// Contact write status: the outcome of the most recent create or edit (pulled via
    /// `MailcalApp::contact_write_status`); drives the editor's "saving…" state and the
    /// message a refused or unconfirmed write earns.
    ContactsStatus,
    /// A pending question about an invitation reply the calendar server could not deliver
    /// (pulled via `MailcalApp::reply_prompt`); drives the modal offering to email the
    /// organiser ourselves. `None` means there is nothing to ask.
    InvitationReply,
    /// A message that was sent but whose copy is not in the account's Sent folder (pulled via
    /// `MailcalApp::unfiled_copy`); drives the modal offering to file it. Unlike `Sending`
    /// this does **not** auto-clear; it stands until the user answers.
    UnfiledCopy,
}

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

/// A foreign (Kotlin/Swift) observer the app notifies when a surface changes; the
/// host then pulls the new snapshot. Must be cheap and non-blocking.
#[uniffi::export(callback_interface)]
pub trait Observer: Send + Sync {
    /// Signals that `surface`'s snapshot changed.
    fn surface_changed(&self, surface: Surface);
}
