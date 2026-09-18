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
    /// Withdraw a queued send so it is never delivered.
    ///
    /// Refused by the store while the send is actually in flight, because stopping
    /// something already on its way is not a promise this app can keep.
    Cancel(QueuedRef),
    /// Attempt a queued send now, rather than waiting out its backoff.
    ///
    /// Does **not** reset the attempt count: one more attempt now is not a fresh bound, so
    /// holding the button cannot outrun the retry limit.
    SendNow(QueuedRef),
    /// Withdraw a queued send and reopen it in the composer.
    ///
    /// Withdrawing first is what makes this safe: the message leaves the queue before the
    /// composer opens, so a drain running in the same moment cannot deliver the copy the
    /// user is editing. Pressing Send in the composer queues a **new** op.
    Edit(QueuedRef),
}
