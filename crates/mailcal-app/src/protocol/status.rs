//! The terminal-state enums a host renders: the outgoing-send hint and the two write hints.
//! Split from [`super`] (the surfaces, observer and intents) to keep each file under the
//! 500-line limit.

// Named only by the doc links below, which rustdoc resolves against this module's scope.
#[allow(unused_imports, reason = "named by intra-doc links on the enums")]
use super::{Intent, Surface};

/// The state of the most recent calendar write (create / edit / delete), surfaced via
/// [`Surface::CalendarStatus`] so a host can show the user that their action took: a small
/// spinner while it settles, a warning when the local view could not be confirmed against
/// the server.
///
/// The lifecycle mirrors [`SendStatus`]: a write moves it to [`Saving`](Self::Saving), then
/// to a terminal [`Saved`](Self::Saved) or [`Failed`](Self::Failed). A later write, or a
/// full [`Intent::RefreshCalendar`], returns it to [`Idle`](Self::Idle).
///
/// **[`Failed`](Self::Failed) does not mean "the save was lost."** A write whose server call
/// succeeded but whose post-write reconcile came back `Busy`/`Failed` has **landed on the
/// server**: only the local copy is briefly stale, and the next sync heals it (the core never
/// re-issues the write). `Failed` here means "we could not confirm the local view is current,"
/// which is what the warning icon should say: not "your change was rejected."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarWriteStatus {
    /// No calendar write is settling.
    #[default]
    Idle,
    /// A create/edit/delete is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The most recent write's server call failed, or its reconcile could not be confirmed
    /// (the change may still have landed; see the type docs).
    Failed,
}

/// The state of the most recent contact write (create or edit), surfaced via
/// [`Surface::ContactsStatus`].
///
/// The same lifecycle as [`CalendarWriteStatus`], and the same warning attached to
/// [`Failed`](Self::Failed): a write whose server call succeeded but whose post-write
/// reconcile came back `Busy`/`Failed` **has landed on the server**. Only the local copy is
/// briefly stale, and the next sync heals it. A separate type from the calendar's because a
/// separate surface signals it: a client on the contacts screen must not spin because a
/// calendar write is settling somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContactWriteStatus {
    /// No contact write is settling.
    #[default]
    Idle,
    /// A create or edit is in flight, or its reconcile is being retried.
    Saving,
    /// The most recent write settled and the local view holds the server's copy.
    Saved,
    /// The most recent write's server call failed, or its reconcile could not be confirmed
    /// (the change may still have landed; see the type docs).
    Failed,
    /// The write was refused **before** anything was sent: the edit named nothing to file the
    /// card under, or carried a value that is not an email address.
    ///
    /// Distinct from [`Failed`](Self::Failed) because the two are different sentences on
    /// screen. `Failed` is "we could not save this, try again"; this is "there is something
    /// to correct first", and retrying the same form would fail the same way.
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
    /// The most recent submission failed: the message did **not** go out.
    Failed,
}
