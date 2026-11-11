//! The *out* half of the unidirectional loop: the [`Surface`]s a host observes and the
//! [`Observer`] callback the app signals them through.
//!
//! Beside `protocol.rs` rather than in it, which holds the *in* half (the intents). The two
//! are read at different moments (a client wires the observer once and dispatches intents
//! forever), and together they crossed the 500-line limit.

/// A surface a host observes and pulls a snapshot for.
#[derive(uniffi::Enum)]
pub enum Surface {
    /// The mailbox/message list.
    MailboxList,
    /// The calendar agenda.
    Calendar,
    /// The settings surface: the active display timezone and any pending change.
    Settings,
    /// What the person's other devices have to say (pulled via `MailcalApp::allodia_devices`):
    /// a pass starting or ending. Nothing else moves with it, so a host redraws only the
    /// Accounts card and the first-run card, which also follow `Settings` for each Keep.
    OtherDevices,
    /// The reading view: the open message's fetched, sanitised body.
    Reading,
    /// The outgoing-send status; drives the composer's "sending…" → "sent" hint.
    Sending,
    /// Background mail-download progress; drives a "downloading Y of X" bar (pulled via
    /// `MailcalApp::sync_progress`).
    SyncProgress,
    /// Calendar write status: the outcome of the most recent create/edit/delete (pulled via
    /// `MailcalApp::calendar_write_status`); drives a small in-calendar spinner and warning.
    CalendarStatus,
    /// The contacts list: the unified people snapshot (pulled via `MailcalApp::contact_list`).
    Contacts,
    /// Contact write status: the outcome of the most recent create or edit (pulled via
    /// `MailcalApp::contact_write_status`); drives the editor's "saving…" state and the
    /// message a refused or unconfirmed write earns.
    ContactsStatus,
    /// A queued send the user asked to edit, waiting to be opened in the host's composer
    /// (pulled via `MailcalApp::compose_request`). It does **not** auto-clear: the message has
    /// already left the Outbox, so this is the only copy of it, and the host clears it once its
    /// composer holds it.
    ComposeRequest,
    /// Draft save status: how the most recent save of the message being composed ended
    /// (pulled via `MailcalApp::draft_status`); drives the composer's quiet "saved" hint.
    DraftStatus,
    /// The notices (pulled via `MailcalApp::notices`; `docs/notices.md`). Signalled whenever
    /// what the app can reach changes, whenever the core raises or settles a question, and
    /// whenever a notice is raised, closed or marked otherwise.
    Notices,
}

/// A foreign (Kotlin/Swift) observer the app notifies when a surface changes; the
/// host then pulls the new snapshot. Must be cheap and non-blocking.
#[uniffi::export(callback_interface)]
pub trait Observer: Send + Sync {
    /// Signals that `surface`'s snapshot changed.
    fn surface_changed(&self, surface: Surface);
}
