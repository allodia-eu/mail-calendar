//! The host-facing protocol of the unidirectional loop: the [`Surface`]s a host
//! observes, the [`AppObserver`] it implements to learn one changed, and the
//! [`Intent`] it dispatches. Split out of `lib.rs` to keep it under the 500-line
//! limit; these are the pure types the [`crate::App`] runtime drives.

use mailcal_composer::DraftBlobHandle;

// The two terminal-state enums a host renders: the send hint and the calendar write hint;
// and the intent enum itself. Their own files so this one stays under the 500-line limit;
// `lib.rs` re-exports every half, so the split is invisible to a host.
mod intent;
mod status;

pub use intent::Intent;
pub use status::{CalendarWriteStatus, SendStatus};

/// A surface a host observes and pulls an immutable snapshot for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The mailbox/message list.
    MailboxList,
    /// The calendar agenda.
    Calendar,
    /// The settings surface; currently the active display timezone and any pending
    /// device-zone change the host prompts on.
    Settings,
    /// The reading view: the open message's fetched, sanitised body.
    Reading,
    /// The outgoing-send status; drives the composer's "sending…" → "sent" hint.
    Sending,
    /// Background mail-download progress; drives a "downloading Y of X" bar while a
    /// sync is running (the snapshot is pulled via `App::sync_progress`).
    SyncProgress,
    /// Connectivity; whether the device is offline and which accounts can't reach their
    /// server (the snapshot is pulled via `App::connectivity`); drives the offline banner
    /// and per-account outage badges.
    Connectivity,
    /// Calendar write status: the outcome of the most recent create/edit/delete (pulled
    /// via `App::calendar_write_status`); drives a small in-calendar spinner while a write
    /// is settling and a warning icon when its reconcile could not be confirmed.
    CalendarStatus,
    /// The contacts list: the unified people snapshot (pulled via `App::contacts`).
    /// Signalled after a contacts sync and after a search narrows the list.
    Contacts,
    /// A pending question about an invitation reply the calendar server could not deliver
    /// (pulled via `App::reply_prompt`); drives the modal that offers to email the organizer
    /// ourselves. `None` means there is nothing to ask.
    InvitationReply,
    /// A message that was sent but whose copy is not in the account's Sent folder (pulled via
    /// `App::unfiled_copy`); drives the modal that offers to file it. Unlike
    /// [`Self::Sending`] this does **not** auto-clear: it is a standing question, and the
    /// user answers it by retrying or dismissing.
    UnfiledCopy,
}

/// A host implements this to learn a [`Surface`] changed, then pulls its snapshot.
///
/// Implementations must be cheap and non-blocking (e.g. hop to the UI thread and
/// re-render); the app awaits nothing on them.
pub trait AppObserver: Send + Sync {
    /// Signals that `surface`'s snapshot changed.
    fn surface_changed(&self, surface: Surface);
}

/// Host-resolved bytes for one composer blob handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerBlob {
    /// The opaque blob handle emitted by the composer output.
    pub handle: DraftBlobHandle,
    /// The attachment bytes for that handle.
    pub bytes: Vec<u8>,
}

impl ComposerBlob {
    /// Builds a resolved composer blob.
    #[must_use]
    pub fn new(handle: DraftBlobHandle, bytes: Vec<u8>) -> Self {
        Self { handle, bytes }
    }
}

/// Suggested recipients for a reply or reply-all, for a host to pre-fill the composer's
/// editable `To`/`Cc` fields (which the user may then edit before sending). Each is a
/// comma-separated address list. A plain reply has an empty `cc`; both are empty when the
/// original message can't be resolved. Produced by [`crate::App::reply_recipients`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipientSuggestion {
    /// The suggested `To` field: the original's `Reply-To`, else its `From`.
    pub to: String,
    /// The suggested `Cc` field: for a reply-all, the other original recipients; empty
    /// for a plain reply.
    pub cc: String,
}

/// Which folders an active search covers.
///
/// Search answers "where is that message", so it looks **everywhere** by default; every
/// account, every folder, and offers narrowing as a filter, the way Outlook does. The one
/// folder left out of the default is Trash: a message the user threw away is not what they
/// are looking for, and a deleted copy sitting beside the live one is noise. Trash is still
/// reachable; open it and search [`CurrentFolder`](Self::CurrentFolder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchScope {
    /// Every account, every folder, except each account's Trash. The default.
    #[default]
    AllFolders,
    /// Only what the mailbox list was showing when the search started: the selected folder,
    /// or (in the unified view) every account's Inbox. Mirrors the list exactly, so
    /// searching *inside* Trash (or an account's all-mail view) finds trashed mail.
    CurrentFolder,
}
