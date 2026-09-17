//! The Outbox: the sends that have not gone, projected for the folder pane and the list.
//!
//! A queued send is not a stored message, and the two must not share a row type. It has no
//! provider key (nothing on a server has it yet), no sender to show (it is the user), no read
//! or flagged state, and no received date. Its identity is the durable outbox op, and the
//! interesting facts about it are the ones a stored message never has: how many attempts it
//! has had, and why it has not gone.
//!
//! The list is projected in **every** view, because the pane's Outbox row carries its count
//! and the pane is always on screen (`docs/folder-pane.md`, rule 1). An empty list is what
//! takes the row off screen entirely.

use engine_api::{PendingOpRow, PendingOpState, queued_draft};

use crate::sender::address_label;

/// Where a queued send has got to.
///
/// Deliberately coarser than the store's `PendingOpState`: a host shows a person what is
/// happening to their message, not the lifecycle of an op. `Pending` covers both a send that
/// has never been tried and one waiting out a backoff, because the difference is a delay the
/// user did not choose and cannot act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuedState {
    /// Waiting to go: queued, or backing off between attempts.
    Waiting,
    /// A send is in flight right now.
    Sending,
    /// The send may or may not have been delivered, and will **never** be retried
    /// automatically. Only a reconcile or the user resolves it.
    ///
    /// Its own state because it is the one queued row where "try again" is the wrong
    /// offer: the message may already be in front of its recipients.
    Unconfirmed,
}

/// One queued send, as the Outbox shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedRow {
    /// The account it will be sent from.
    pub account: String,
    /// The durable op's id: the row's identity, and what an action names.
    pub op: u64,
    /// The recipients, comma-joined for display. Empty if the draft carried none.
    pub to: String,
    /// The subject (empty if none).
    pub subject: String,
    /// Where the send has got to.
    pub state: QueuedState,
    /// How many attempts have been made. `0` means it has not been tried yet.
    pub attempts: u32,
    /// Why the last attempt did not go through, as a provider detail, or `None` when
    /// nothing has failed yet.
    ///
    /// A class and protocol detail, never draft content: this reaches a log line and a
    /// host-visible hint (`docs/logging.md`).
    pub detail: Option<String>,
}

/// Projects an account's outstanding outbox rows into the Outbox list.
///
/// Only **sends** appear. A queued archive or flag change is a write the user made on a
/// message that is already in a folder, and the folder is where they will look for it; a
/// row for it in a list of unsent mail would be a second, stranger copy of the same action.
/// Those drain in the background and are never shown here.
#[must_use]
pub fn queued_rows(account: &str, ops: &[PendingOpRow]) -> Vec<QueuedRow> {
    ops.iter()
        .filter_map(|row| queued_row(account, row))
        .collect()
}

fn queued_row(account: &str, row: &PendingOpRow) -> Option<QueuedRow> {
    // `queued_draft` refuses anything that is not a submission, which is also the filter
    // that keeps edits and reports out of the list.
    let draft = queued_draft(row)?;
    let state = match row.state {
        PendingOpState::Pending => QueuedState::Waiting,
        PendingOpState::InFlight => QueuedState::Sending,
        PendingOpState::NeedsConfirmation => QueuedState::Unconfirmed,
        // The queue read returns nothing settled, so these are unreachable in practice;
        // dropping the row is the harmless reading either way.
        PendingOpState::Succeeded | PendingOpState::Failed | PendingOpState::Cancelled => {
            return None;
        }
    };
    Some(QueuedRow {
        account: account.to_owned(),
        op: row.id.get(),
        to: draft
            .to
            .iter()
            .map(|to| address_label(to.name.as_deref(), &to.email))
            .collect::<Vec<_>>()
            .join(", "),
        subject: draft.subject.clone(),
        state,
        attempts: row.attempts,
        detail: row.detail.clone(),
    })
}
