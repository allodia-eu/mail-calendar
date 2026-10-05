//! The Outbox half of the FFI intent surface.
//!
//! Nested behind one `Intent::Outbox` variant rather than flattened into [`Intent`] the way
//! the contacts intents are. Not a change of heart about the shape: [`Intent`] is a single
//! enum and single enums cannot be split across files, so the family that would have pushed
//! `intent.rs` past the 500-line limit is the family that nests. It mirrors the core's own
//! `OutboxIntent`, so the two sides read alike.

use uniffi::Enum;

/// What the Outbox surface asks the app to do.
///
/// Every action names a queued send by **both** its account and its op id. An op id is
/// unique only within its account's outbox, and the Outbox holds every account's sends at
/// once, so an id on its own would be resolved against whichever account happened to be
/// selected.
#[derive(Debug, Clone, PartialEq, Eq, Enum)]
pub enum OutboxIntent {
    /// Show the Outbox: every account's unsent messages, in one list.
    Show,
    /// Withdraw a queued send so it is never delivered, or dismiss one that was not sent.
    ///
    /// Refused while the send is actually in flight, or awaiting confirmation; the app says so
    /// and the row stays.
    Cancel {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
    /// Attempt a queued send now instead of waiting out its backoff, or send again one that
    /// was not sent.
    ///
    /// Never sends one awaiting confirmation: that takes `ConfirmNotSent`.
    SendNow {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
    /// Move a queued send back into Drafts and open it in this client's composer.
    ///
    /// The app saves it as a draft, withdraws it from the Outbox, then raises
    /// `Surface::ComposeRequest` carrying the message, its files and the composition to open
    /// on; a host opens its composer from that and dismisses the request. Pressing Send there
    /// queues a new message. Offered only where `QueuedRow::editable`.
    Edit {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
        /// The host's staging directory, where the message's files are written for the
        /// composer to attach, as for `resume_draft`.
        staging_directory: String,
    },
    /// Answer a send awaiting confirmation: it reached its recipients. It leaves the Outbox and
    /// is never sent again.
    ConfirmSent {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
    /// Answer a send awaiting confirmation: it did not reach them, so send it now.
    ///
    /// The user's own answer, after checking; it may deliver the message a second time if
    /// they were wrong, which is why no client offers it on any other state and the app
    /// refuses it on one.
    ConfirmNotSent {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
}
