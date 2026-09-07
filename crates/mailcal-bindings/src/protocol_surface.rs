//! The *out* half of the unidirectional loop: the [`Surface`]s a host observes and the
//! [`Observer`] callback the app signals them through.
//!
//! Beside `protocol.rs` rather than in it, which holds the *in* half (the intents). The two
//! are read at different moments — a client wires the observer once and dispatches intents
//! forever — and together they crossed the 500-line limit.

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

/// A foreign (Kotlin/Swift) observer the app notifies when a surface changes; the
/// host then pulls the new snapshot. Must be cheap and non-blocking.
#[uniffi::export(callback_interface)]
pub trait Observer: Send + Sync {
    /// Signals that `surface`'s snapshot changed.
    fn surface_changed(&self, surface: Surface);
}
