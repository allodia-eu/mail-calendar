//! The terminal-state enums a host renders: the outgoing-send hint and the two write hints.
//! Split from [`super`] (the surfaces, observer and intents) to keep each file under the
//! 500-line limit.

// Named only by the doc links below, which rustdoc resolves against this module's scope.
#[allow(unused_imports, reason = "named by intra-doc links on the enums")]
use super::{Intent, Surface};

/// The state of the most recent calendar write (create / edit / delete), surfaced via
/// [`Surface::CalendarStatus`] so a host can show the user that their action took: a small
/// spinner while it settles, and a warning when it did not save or could not be confirmed.
///
/// The lifecycle mirrors [`SendStatus`]: a write moves it to [`Saving`](Self::Saving), then
/// to a terminal [`Saved`](Self::Saved), [`NotSaved`](Self::NotSaved) or
/// [`Unconfirmed`](Self::Unconfirmed). A later write, or a full [`Intent::RefreshCalendar`],
/// returns it to [`Idle`](Self::Idle).
///
/// **The two failures are two sentences.** [`NotSaved`](Self::NotSaved) is a write that changed
/// nothing on the server. [`Unconfirmed`](Self::Unconfirmed) may have landed: a request whose
/// answer was lost, or a write whose server call succeeded but whose post-write reconcile came
/// back `Busy`/`Failed`, which has **landed on the server** with only the local copy briefly
/// stale (the core never re-issues the write).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarWriteStatus {
    /// No calendar write is settling.
    #[default]
    Idle,
    /// A create/edit/delete is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The change was not saved: nothing reached the server (the calendar is not connected, or
    /// the core refused the change before sending it), or the server answered and refused it.
    NotSaved,
    /// The change may have landed and could not be confirmed: the connection failed or the
    /// server erred after the write was sent, or the write landed and the copy read back to
    /// confirm it did not arrive. The next sync settles which.
    Unconfirmed,
}

/// The state of the most recent contact write (create or edit), surfaced via
/// [`Surface::ContactsStatus`].
///
/// The same lifecycle as [`CalendarWriteStatus`], and the same line between its two failures:
/// [`NotSaved`](Self::NotSaved) when nothing reached the server or the server refused it, and
/// [`Unconfirmed`](Self::Unconfirmed) when the change may have landed. A separate type from the
/// calendar's because a separate surface signals it: a client on the contacts screen must not
/// spin because a calendar write is settling somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContactWriteStatus {
    /// No contact write is settling.
    #[default]
    Idle,
    /// A create or edit is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The change was not saved: nothing reached the server (its account or address book is
    /// not connected), or the server answered and refused it.
    NotSaved,
    /// The change may have landed and could not be confirmed: the connection failed or the
    /// server erred after the write was sent, or the write landed and the copy read back to
    /// confirm it did not arrive. The next sync settles which.
    Unconfirmed,
    /// The write was refused **before** anything was sent: the edit named nothing to file the
    /// card under, or carried a value that is not an email address.
    ///
    /// Distinct from [`NotSaved`](Self::NotSaved) because the two are different sentences on
    /// screen: this is "there is something to correct first", and retrying the same form would
    /// fail the same way.
    Invalid,
}

/// The state of the most recent outgoing send, surfaced via [`Surface::Sending`] so a host
/// can show a "sending…" → "sent" hint. Starting a new send resets it to
/// [`SendStatus::Sending`]; the host shows the terminal state briefly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SendStatus {
    /// No send has started this session.
    #[default]
    Idle,
    /// A validated message is being submitted through the outbox.
    Sending,
    /// The most recent submission completed, and a copy is in the account's Sent folder.
    Sent,
    /// The message **was sent**, but the copy for the account's Sent folder could not be
    /// filed; it is not there and will not appear later.
    ///
    /// Neither [`Self::Sent`] (which is how the copy came to be lost in silence) nor
    /// [`Self::Failed`] (telling someone a send failed when it did not is how a message gets
    /// sent twice). The standing, actionable form of this is [`Surface::UnfiledCopy`], only
    /// IMAP/SMTP accounts reach it, since every other transport files the copy within the send.
    SentNotFiled,
    /// The most recent submission has **not gone yet**, and is waiting in the Outbox.
    ///
    /// Not [`Self::Failed`]: the message is not lost, and telling someone a send failed when
    /// it is queued invites them to write it again. The standing form of this is the pane's
    /// Outbox row, which stays until the message goes; this hint auto-clears like the others.
    Queued,
    /// The message **may have been delivered**: the server stopped answering after it could
    /// act on it. It is in the Outbox awaiting the user's answer, and is never sent again on
    /// its own.
    ///
    /// Neither [`Self::Queued`] (it will not go out by itself) nor [`Self::Failed`] (a re-send
    /// may deliver it twice).
    Unconfirmed,
    /// The server **refused** the message: it did not go out, and nothing will retry it. It
    /// is in the Outbox, its one place now, until the user sends it again, edits it or
    /// discards it, so the hint says where it is rather than only that it failed.
    NotSent,
    /// The most recent submission failed before the Outbox held it: the message did **not**
    /// go out, and only the composer's draft keeps it.
    Failed,
}

/// The state of the most recent draft save, surfaced via [`Surface::DraftStatus`] so a
/// composer can say quietly that the words are on the server.
///
/// **A hint, never a gate.** Saving a draft is not something a user waits for: no state here
/// blocks a composer from being closed, and none is worth a modal. That is what separates it
/// from [`SendStatus`], whose terminal states answer a question the user asked out loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DraftStatus {
    /// Nothing has been saved this session.
    #[default]
    Idle,
    /// A save is in flight.
    Saving,
    /// The draft is on the server. Also the answer to a save whose content was unchanged,
    /// which reaches no server and is still honestly "saved" (`docs/drafts.md`).
    Saved,
    /// The save has **not** reached the server yet, and is waiting in the outbox for a
    /// network.
    ///
    /// Not [`Self::Failed`]: the words are not lost, and the queued save goes out by itself.
    /// The distinction is the one [`SendStatus::Queued`] draws, for the same reason.
    Queued,
    /// The save did not reach the server and nothing will retry it.
    Failed,
}
