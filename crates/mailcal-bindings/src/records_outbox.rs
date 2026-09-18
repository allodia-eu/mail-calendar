//! The Outbox's FFI records: one queued send, and the message the core asks a host to open.
//!
//! Split from [`records`](crate::records), which is at the 500-line limit.

use mailcal_viewmodel::{QueuedRow as AppQueuedRow, QueuedState as AppQueuedState};

/// Where a queued send has got to.
///
/// Coarser than the engine's own op states on purpose: a person is shown what is happening
/// to their message, not the lifecycle of a durable operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum QueuedState {
    /// Waiting to go: queued, or backing off between attempts. The ordinary state, and the
    /// one a host shows plainly rather than as a warning.
    Waiting,
    /// Being sent right now. A host disables the row's actions: nothing can be withdrawn or
    /// hurried while it is on its way.
    Sending,
    /// It may or may not have been delivered, and will **never** be retried automatically.
    ///
    /// The one queued row where "send now" is the wrong offer: the message may already be in
    /// front of its recipients, so a host shows this state and offers no retry.
    Unconfirmed,
}

/// One unsent message, as the Outbox shows it.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct QueuedRow {
    /// The account it will be sent from.
    pub account: String,
    /// The queued send's op id: what `OutboxIntent`'s actions name, together with `account`.
    pub op: u64,
    /// The recipients, comma-joined and ready to draw.
    pub to: String,
    /// The subject (empty if none).
    pub subject: String,
    /// Where it has got to.
    pub state: QueuedState,
    /// How many attempts have been made; `0` means it has not been tried yet.
    ///
    /// A host may show this once it climbs, to explain a message that is taking a while. It
    /// is not an error count: the first attempt failing is the ordinary case this whole
    /// surface exists for.
    pub attempts: u32,
    /// The failure class and protocol detail of the last attempt, or `None`.
    ///
    /// **Not for the row.** What a user reads is plain language the client writes; this is
    /// carried for the diagnostics screen and support, like `UnfiledCopy::detail`.
    pub detail: Option<String>,
}

/// A message the core asks the host to open in its composer, unsent.
///
/// Raised when a user edits a queued send: the core withdraws it from the Outbox first, so
/// by the time this exists the message is **nowhere else**. A host opens its composer with
/// these fields and then dismisses the request (`Intent::DismissComposeRequest`).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ComposeRequest {
    /// The account to send from.
    pub account: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    pub body_text: String,
}

impl From<AppQueuedState> for QueuedState {
    fn from(state: AppQueuedState) -> Self {
        match state {
            AppQueuedState::Waiting => Self::Waiting,
            AppQueuedState::Sending => Self::Sending,
            AppQueuedState::Unconfirmed => Self::Unconfirmed,
        }
    }
}

impl From<AppQueuedRow> for QueuedRow {
    fn from(row: AppQueuedRow) -> Self {
        Self {
            account: row.account,
            op: row.op,
            to: row.to,
            subject: row.subject,
            state: row.state.into(),
            attempts: row.attempts,
            detail: row.detail,
        }
    }
}

impl From<mailcal_app::ComposeRequest> for ComposeRequest {
    fn from(request: mailcal_app::ComposeRequest) -> Self {
        Self {
            account: request.account,
            to: request.to,
            cc: request.cc,
            bcc: request.bcc,
            subject: request.subject,
            body_text: request.body_text,
        }
    }
}
