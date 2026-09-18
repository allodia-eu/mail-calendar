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
    /// Withdraw a queued send so it is never delivered.
    ///
    /// Refused while the send is actually in flight; the app says so and the row stays.
    Cancel {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
    /// Attempt a queued send now instead of waiting out its backoff.
    ///
    /// Does not reset the attempt count, so holding the button cannot outrun the retry
    /// limit.
    SendNow {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
    /// Withdraw a queued send and reopen it in this client's composer.
    ///
    /// The app withdraws it **first**, then raises `Surface::ComposeRequest` carrying the
    /// message; a host opens its composer from that and dismisses the request. Pressing Send
    /// there queues a new message.
    Edit {
        /// The account whose outbox holds it.
        account: String,
        /// The queued send's op id, from `QueuedRow::op`.
        op: u64,
    },
}
