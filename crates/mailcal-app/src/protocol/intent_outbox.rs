//! The Outbox half of the inbound protocol ([`OutboxIntent`]), which [`Intent::Outbox`]
//! carries.
//!
//! Split from [`super::intent`] to keep each file under the 500-line limit, following
//! [`ContactsIntent`](super::ContactsIntent). The Outbox reads as a unit for the same
//! reason contacts do: one surface, one list, and three actions that exist nowhere else
//! because no other row in the app names a message the server has never seen.

use crate::reference::QueuedRef;

/// What the Outbox surface asks the runtime to do.
///
/// Every action names a [`QueuedRef`]: an account bound to the durable op in its queue. An
/// op id is unique only within its account's outbox, and the pane's Outbox row holds every
/// account's sends at once, so a bare id would be resolved against whichever account
/// happened to be selected (`docs/folder-pane.md`, rule 14, for the same reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxIntent {
    /// Show the Outbox: the sends that have not gone, across every account.
    Show,
    /// Withdraw a queued send so it is never delivered, or dismiss one that was not sent.
    ///
    /// Refused by the store while the send is actually in flight, because stopping
    /// something already on its way is not a promise this app can keep, and while it awaits
    /// confirmation, because it may already have been delivered.
    Cancel(QueuedRef),
    /// Attempt a queued send now, rather than waiting out its backoff, or send again one the
    /// server refused.
    ///
    /// Does **not** reset the attempt count: one more attempt now is not a fresh bound. Never
    /// sends a message awaiting confirmation; only [`ConfirmNotSent`](Self::ConfirmNotSent)
    /// does, so a row that changed state under the user's click cannot deliver it twice.
    SendNow(QueuedRef),
    /// Move a queued send back into Drafts and open it in the composer, holding its files.
    ///
    /// The draft is saved before the send leaves the queue, and the send leaves the queue
    /// before the composer opens: an app that ends part way leaves the message in both
    /// places, never in neither, and a drain cannot deliver the copy being edited. Pressing
    /// Send in the composer queues a **new** op.
    Edit {
        /// The queued send.
        queued: QueuedRef,
        /// Where the host's composer reads attachments from; the message's files are written
        /// there before the composer is offered.
        staging_directory: String,
    },
    /// The user's answer to a send whose delivery could not be confirmed: it reached its
    /// recipients. It settles and leaves the Outbox, and is never sent again.
    ConfirmSent(QueuedRef),
    /// The user's answer to a send whose delivery could not be confirmed: it did not reach
    /// them. It goes back in the queue and is sent now.
    ///
    /// Refused by the store on a send that is not awaiting confirmation, so this is the one
    /// way a message that may have been delivered is ever attempted again.
    ConfirmNotSent(QueuedRef),
}
