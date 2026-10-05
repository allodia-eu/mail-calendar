//! What the Outbox writes to the log: each queued write a drain pass attempted and what became
//! of it, each answer a person gives a queued send, and what a start-up found left over.
//!
//! A queued send goes out with nobody watching, so the log is the only record of what happened
//! to it (`docs/logging.md`). Every path that changes one leaves a line. None carries anything of
//! the message: an account is its ordinal, a queued write its number in the queue, and a failure
//! its class and the server's own words.

use engine_api::{
    AccountId, DrainOutcome, DrainReport, OpRejection, PendingOpId, PendingOpKind, PendingOpRow,
    PendingOpState, Provider,
};
use engine_core::error::FailureClass;

use crate::App;

/// What a queued write is, in the words a log reader uses.
fn kind_words(kind: PendingOpKind) -> &'static str {
    match kind {
        PendingOpKind::MailSubmit => "send",
        PendingOpKind::MailEdit => "message change",
        PendingOpKind::MailReport => "spam report",
        PendingOpKind::MailDraftPut => "draft save",
        PendingOpKind::MailDraftDelete => "draft removal",
        PendingOpKind::MailboxEdit => "folder change",
        PendingOpKind::CalendarRsvp => "invitation reply",
        PendingOpKind::CalendarCreate
        | PendingOpKind::CalendarPatch
        | PendingOpKind::CalendarDocument
        | PendingOpKind::CalendarDelete => "calendar change",
        PendingOpKind::ContactCreate
        | PendingOpKind::ContactPatch
        | PendingOpKind::ContactDelete => "contact change",
    }
}

/// Why an attempt did not go through, in words.
fn class_words(class: FailureClass) -> &'static str {
    match class {
        FailureClass::Retryable => "the server could not be reached or did not answer",
        FailureClass::RateLimited => "the server asked to slow down",
        FailureClass::Authentication => "the server refused the sign-in",
        FailureClass::Conflict => "the server's copy had changed",
        FailureClass::InvalidState => "the server cannot do this",
        FailureClass::NeedsResync => "the server's state had moved on",
        FailureClass::Permanent => "the server refused it",
        _ => "an unclassified failure",
    }
}

/// The reason a person's action on a queued send did not take effect.
pub(crate) fn refusal_words(refusal: OpRejection) -> &'static str {
    match refusal {
        OpRejection::Unknown => "it is no longer queued",
        OpRejection::Settled => "it has already finished",
        OpRejection::InFlight => "it is being sent right now",
        OpRejection::AwaitingConfirmation => "it may already have been delivered",
        OpRejection::NotAwaitingConfirmation => "nothing is waiting on that answer",
    }
}

/// Logs each write a drain pass attempted, and its outcome.
///
/// `after` is the queue read once the pass ended. A write that did not go through is still in
/// it, carrying the server's detail, which the pass's own report does not.
pub(crate) fn log_drain_report(acct: usize, report: &DrainReport, after: &[PendingOpRow]) {
    for op in &report.attempted {
        let what = kind_words(op.kind);
        let id = op.id.get();
        let detail = after
            .iter()
            .find(|row| row.id == op.id)
            .and_then(|row| row.detail.as_deref())
            .unwrap_or("no detail");
        match &op.outcome {
            DrainOutcome::Succeeded => {
                log::info!("outbox[a{acct}]: queued {what} {id} went through");
            }
            DrainOutcome::SentNotFiled { detail } => log::warn!(
                "outbox[a{acct}]: queued send {id} was delivered, but its copy was not filed in \
                 Sent: {detail}"
            ),
            DrainOutcome::Parked { class, attempts } => log::info!(
                "outbox[a{acct}]: queued {what} {id} did not go through on attempt {attempts}, \
                 {}; it stays queued: {detail}",
                class_words(*class)
            ),
            DrainOutcome::Failed { class } if op.kind == PendingOpKind::MailSubmit => log::warn!(
                "outbox[a{acct}]: queued send {id} was not sent, {}; it stays in the Outbox \
                 until it is sent again or discarded: {detail}",
                class_words(*class)
            ),
            DrainOutcome::Failed { class } => log::warn!(
                "outbox[a{acct}]: queued {what} {id} did not go through, {}, and will not be \
                 tried again",
                class_words(*class)
            ),
            DrainOutcome::AwaitingConfirmation { detail } => log::warn!(
                "outbox[a{acct}]: queued send {id} may have been delivered, but the server did not \
                 confirm it; it will not be sent again unless the user says it did not arrive: \
                 {detail}"
            ),
            DrainOutcome::Undecodable { detail } => log::error!(
                "outbox[a{acct}]: queued {what} {id} could not be read by this version of the app \
                 and was dropped: {detail}"
            ),
        }
    }
    if report.deferred > 0 {
        log::info!(
            "outbox[a{acct}]: {} queued write(s) left for a later pass",
            report.deferred
        );
    }
}

/// Logs a person's action on a queued send that took effect.
pub(crate) fn log_action(acct: usize, op: PendingOpId, done: &str) {
    log::info!("outbox[a{acct}]: queued send {}: {done}", op.get());
}

/// Logs why a person's action on a queued send did not take effect.
///
/// Every refusal is a state the user can see resolve itself, or one the row already explains,
/// so the app says what happened and rebuilds rather than raising a question.
pub(crate) fn log_refusal(acct: usize, op: PendingOpId, action: &str, refusal: OpRejection) {
    log::info!(
        "outbox[a{acct}]: could not {action} queued send {}: {}",
        op.get(),
        refusal_words(refusal)
    );
}

impl<P: Provider> App<P> {
    /// Puts back what the previous run left unfinished in the outbox, and says what is in it.
    ///
    /// Called once at start-up, before any drain or write runs. A send that run was cut off
    /// before it handed the message to the server is retried at once; one cut off after it may
    /// already have been delivered, so it waits for an answer and is never sent again on its own
    /// (`docs/sending.md`). Without this the next run would still recover them, but only once
    /// their lease lapsed, minutes later.
    pub async fn recover_outbox(&self) {
        match self.engine.recover_interrupted_ops().await {
            Ok(0) => {}
            Ok(count) => log::warn!(
                "outbox: {count} queued write(s) were cut off when the app last stopped, and \
                 were put back"
            ),
            Err(err) => log::error!(
                "outbox: could not recover the writes the app left unfinished when it last \
                 stopped; they are retried once their attempt expires: {err}"
            ),
        }
        for account in self.account_ids().await {
            self.log_outbox_contents(&account).await;
        }
    }

    /// One line per account whose Outbox holds anything, counted by state.
    async fn log_outbox_contents(&self, account: &AccountId) {
        let rows = match self.engine.outbox(account).await {
            Ok(rows) => rows,
            Err(err) => {
                log::warn!("outbox: could not read the queue at start-up: {err}");
                return;
            }
        };
        let sends: Vec<&PendingOpRow> = rows
            .iter()
            .filter(|row| row.kind == Some(PendingOpKind::MailSubmit))
            .collect();
        let others = rows.len() - sends.len();
        if rows.is_empty() {
            return;
        }
        let count = |state: PendingOpState| sends.iter().filter(|row| row.state == state).count();
        let acct = self.account_ordinal(account).await;
        log::info!(
            "outbox[a{acct}]: at start-up, {} send(s) waiting, {} awaiting confirmation, {} not \
             sent; {others} other queued write(s)",
            count(PendingOpState::Pending) + count(PendingOpState::InFlight),
            count(PendingOpState::NeedsConfirmation),
            count(PendingOpState::Failed),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A log line names the class in words: the enum's own name is our source tree.
    #[test]
    fn every_class_reads_as_words() {
        for class in [
            FailureClass::Retryable,
            FailureClass::RateLimited,
            FailureClass::Authentication,
            FailureClass::Conflict,
            FailureClass::InvalidState,
            FailureClass::NeedsResync,
            FailureClass::Permanent,
        ] {
            let words = class_words(class);
            assert!(!words.contains(&format!("{class:?}")), "{words}");
            assert_ne!(words, "an unclassified failure", "{class:?}");
        }
    }
}
