//! The sending FFI records: the send hint, one queued send, and the message the core asks a host
//! to open.
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
    /// front of its recipients. A host asks the user instead: it was delivered
    /// (`OutboxIntent::ConfirmSent`), or it was not, so send it (`OutboxIntent::ConfirmNotSent`).
    Unconfirmed,
    /// The server refused it, and nothing will try again on its own. It stays until the user
    /// sends it again (`SendNow`), edits it or discards it (`Cancel`).
    NotSent,
}

/// The state of the most recent outgoing send (pulled after a `Surface::Sending` signal).
#[derive(uniffi::Enum)]
pub enum SendStatus {
    /// No send has started this session.
    Idle,
    /// A validated message is being submitted through the outbox.
    Sending,
    /// The most recent submission completed, and a copy is in the account's Sent folder.
    Sent,
    /// The message **was sent**, but its copy could not be filed in the account's Sent
    /// folder; it is not there and will not appear later. Show it as sent, with a warning:
    /// the recipients have the message, only the sender's own record of it is missing.
    /// Never as a failure, that invites a re-send of mail that already went out.
    SentNotFiled,
    /// The submission has **not gone yet** and is waiting in the Outbox; it will be sent
    /// when the network comes back. Show it as pending, never as a failure: the message is
    /// not lost, and telling someone their send failed invites them to write it again.
    /// The standing form of this is the pane's Outbox row.
    Queued,
    /// The message may have reached its recipients: the server stopped answering after it
    /// could act on it. It is in the Outbox, where the user is asked whether it arrived, and
    /// nothing sends it again on its own. Show it as a warning, never as a failure: a re-send
    /// may deliver it twice.
    Unconfirmed,
    /// The server refused it: it did not go out and nothing will retry it. It is in the
    /// Outbox, which the hint names, until the user sends it again, edits it or discards it.
    NotSent,
    /// The most recent submission failed before the Outbox held it: the message did **not**
    /// go out, and only the composer's draft keeps it.
    Failed,
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
    /// Whether Edit is offered. False for a send a composer cannot hold (an invitation's
    /// answer), which a host offers no Edit on.
    pub editable: bool,
}

/// A message that already exists, to open unsent in the host's composer **on `composition`**,
/// so its saves replace the stored copy and its send takes that copy away.
///
/// Two paths answer with it, and a host opens both the same way: `resume_draft` returns one
/// for a draft opened from Drafts, and a user editing a queued send raises one (the core
/// saves it back into Drafts, then withdraws it from the Outbox; the host dismisses it with
/// `Intent::DismissComposeRequest` once open).
///
/// Seed the editor with `setComposerBody({html: body_html, text: body_text})`, which opens the
/// formatting, the pictures, the quoted original and the signature this app wrote, and attach
/// every file in `attachments`.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ComposeRequest {
    /// The account to send from.
    pub account: String,
    /// The composition the draft was saved under: the composer adopts it rather than minting
    /// its own.
    pub composition: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body's HTML: sanitised, with its pictures inline. Empty when the message has none.
    pub body_html: String,
    /// The body as text, which the editor opens when there is no HTML.
    pub body_text: String,
    /// The message's files, already written into the staging directory the host named. The
    /// composer must open holding all of them: its first save replaces the draft, so a file
    /// left out is taken off it.
    pub attachments: Vec<crate::ComposerFileAttachment>,
}

impl From<AppQueuedState> for QueuedState {
    fn from(state: AppQueuedState) -> Self {
        match state {
            AppQueuedState::Waiting => Self::Waiting,
            AppQueuedState::Sending => Self::Sending,
            AppQueuedState::Unconfirmed => Self::Unconfirmed,
            AppQueuedState::NotSent => Self::NotSent,
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
            editable: row.editable,
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
            composition: request.composition,
            subject: request.subject,
            body_html: request.body_html,
            body_text: request.body_text,
            attachments: request
                .attachments
                .into_iter()
                .map(|file| crate::ComposerFileAttachment {
                    path: file.path,
                    file_name: file.file_name,
                    media_type: file.media_type,
                })
                .collect(),
        }
    }
}
