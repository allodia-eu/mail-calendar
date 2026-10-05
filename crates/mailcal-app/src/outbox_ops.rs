//! The Outbox: showing the queued sends, acting on one, and draining them when the network
//! comes back.
//!
//! The engine owns the queue and every decision about it (`store-and-sync.md`): what is due,
//! whether a failure parks or settles, and whether an op can be withdrawn at all. This layer
//! decides only what a *person* is offered, and turns each refusal into something the app can
//! say out loud rather than a silent no-op.

use engine_api::{AccountId, Confirmation, PendingOpKind, PendingOpState, Provider};

use self::outbox_log::{log_action, log_drain_report, log_refusal};
use crate::{App, ComposeRequest, OutboxIntent, Scope, Surface, reference::QueuedRef};

#[path = "outbox_log.rs"]
mod outbox_log;

impl<P: Provider> App<P> {
    /// Routes one [`OutboxIntent`] to its handler.
    pub(crate) async fn dispatch_outbox(&self, intent: OutboxIntent) {
        match intent {
            OutboxIntent::Show => self.show_outbox().await,
            OutboxIntent::Cancel(queued) => self.cancel_queued_send(&queued).await,
            OutboxIntent::SendNow(queued) => self.send_queued_now(&queued).await,
            OutboxIntent::Edit(queued) => self.edit_queued_send(&queued).await,
            OutboxIntent::ConfirmSent(queued) => {
                self.confirm_queued_send(&queued, Confirmation::Delivered)
                    .await;
            }
            OutboxIntent::ConfirmNotSent(queued) => {
                self.confirm_queued_send(&queued, Confirmation::NotDelivered)
                    .await;
            }
        }
    }

    /// Shows the Outbox: the queued sends across every account, in one list.
    pub(crate) async fn show_outbox(&self) {
        *self.scope.lock().expect("scope mutex poisoned") = Scope::Outbox;
        self.rebuild_snapshot().await;
    }

    /// Withdraws a queued send so it is never delivered, or dismisses one that was not sent.
    ///
    /// A refusal is **logged and left on screen**, never swallowed: the row the user pressed
    /// is still there afterwards, and the next rebuild shows why. The refusals that matter are
    /// `InFlight` and `AwaitingConfirmation`, and the honest reading of both is that the message
    /// may already have gone.
    pub(crate) async fn cancel_queued_send(&self, queued: &QueuedRef) {
        let acct = self.account_ordinal(&queued.account).await;
        match self
            .engine
            .cancel_pending_op(&queued.account, queued.op)
            .await
        {
            Ok(None) => log_action(
                acct,
                queued.op,
                "withdrawn by the user; it will not be sent",
            ),
            Ok(Some(refusal)) => log_refusal(acct, queued.op, "withdraw", refusal),
            Err(err) => log::warn!("outbox[a{acct}]: withdrawing a queued send failed: {err}"),
        }
        self.rebuild_snapshot().await;
    }

    /// Attempts a queued send now instead of waiting out its backoff, or sends again one the
    /// server refused, then drains so the attempt happens in this gesture rather than at some
    /// later pass.
    pub(crate) async fn send_queued_now(&self, queued: &QueuedRef) {
        let acct = self.account_ordinal(&queued.account).await;
        match self
            .engine
            .retry_pending_op_now(&queued.account, queued.op)
            .await
        {
            Ok(None) => {
                log_action(acct, queued.op, "the user asked for it to be sent now");
                self.drain_account(&queued.account).await;
            }
            Ok(Some(refusal)) => log_refusal(acct, queued.op, "send now", refusal),
            Err(err) => log::warn!("outbox[a{acct}]: sending a queued send now failed: {err}"),
        }
        self.rebuild_snapshot().await;
    }

    /// Records the user's answer to a send whose delivery could not be confirmed.
    ///
    /// The store refuses an answer to a send that is not waiting for one, so a row that changed
    /// state under the click cannot be sent again through here. An answer that it did not
    /// arrive drains at once, as Send now does.
    pub(crate) async fn confirm_queued_send(&self, queued: &QueuedRef, answer: Confirmation) {
        let acct = self.account_ordinal(&queued.account).await;
        match self
            .engine
            .confirm_pending_op(&queued.account, queued.op, answer)
            .await
        {
            Ok(None) if answer == Confirmation::Delivered => log_action(
                acct,
                queued.op,
                "the user says it was delivered; it will not be sent again",
            ),
            Ok(None) => {
                log_action(
                    acct,
                    queued.op,
                    "the user says it was not delivered; sending it again",
                );
                self.drain_account(&queued.account).await;
            }
            Ok(Some(refusal)) => log_refusal(acct, queued.op, "record an answer for", refusal),
            Err(err) => log::warn!("outbox[a{acct}]: recording the user's answer failed: {err}"),
        }
        self.rebuild_snapshot().await;
    }

    /// Withdraws a queued send and hands it back to the host's composer.
    ///
    /// **Withdraw first, then open.** The other order leaves a window in which a drain pass
    /// delivers the message the user is editing, and there is no taking that back. If the
    /// withdrawal is refused the composer never opens: editing a copy of a message that is
    /// still queued would send it twice.
    pub(crate) async fn edit_queued_send(&self, queued: &QueuedRef) {
        let acct = self.account_ordinal(&queued.account).await;
        let Some(draft) = self.queued_draft(queued).await else {
            log::warn!(
                "outbox[a{acct}]: no queued send {} to edit",
                queued.op.get()
            );
            return;
        };
        match self
            .engine
            .cancel_pending_op(&queued.account, queued.op)
            .await
        {
            Ok(None) => log_action(acct, queued.op, "withdrawn to be edited"),
            Ok(Some(refusal)) => {
                log_refusal(acct, queued.op, "edit", refusal);
                self.rebuild_snapshot().await;
                return;
            }
            Err(err) => {
                log::warn!("outbox[a{acct}]: withdrawing a queued send to edit it failed: {err}");
                return;
            }
        }
        self.raise_compose_request(&queued.account, &draft);
        self.rebuild_snapshot().await;
    }

    /// Attempts every account's queued writes that are **due**, and refreshes if anything
    /// moved.
    ///
    /// Called after a sync, never on a timer: the engine holds no clock for this, and
    /// polling a dead network is what a reachability signal exists to avoid.
    pub(crate) async fn drain_outboxes(&self) {
        for account in self.account_ids().await {
            self.drain_account(&account).await;
        }
    }

    /// Sends everything queued **now**, clearing each message's backoff first.
    ///
    /// The reconnect path. A backoff is the engine's guess at how long to wait for a server
    /// that was not answering, and coming back online is the one piece of information that
    /// makes the guess obsolete: the outage it was waiting out is over. Without this, a
    /// person who reconnects watches their mail sit there for up to half an hour, which is
    /// exactly the wait the Outbox exists to explain rather than impose.
    ///
    /// The attempt counts are untouched: this asks for one more attempt each, now, not for
    /// the retry bound to start again.
    ///
    /// Only what is still waiting is hurried. A send the server refused is in the queue too,
    /// and hurrying it means sending it again: reconnecting is no answer to a refusal, so it
    /// waits for the user rather than going out again on its own.
    pub(crate) async fn flush_outboxes(&self) {
        for account in self.account_ids().await {
            let queued = self.engine.outbox(&account).await.unwrap_or_default();
            for row in queued.iter().filter(|row| {
                matches!(
                    row.state,
                    PendingOpState::Pending | PendingOpState::InFlight
                )
            }) {
                // A refusal here is ordinary (a live attempt) and the drain below is
                // unaffected by it, so it is not worth a line of its own.
                let _ = self.engine.retry_pending_op_now(&account, row.id).await;
            }
            self.drain_account(&account).await;
        }
    }

    /// One account's drain pass, through its first provider.
    async fn drain_account(&self, account: &AccountId) {
        let Some(acct) = self.account_handle(account).await else {
            return;
        };
        let Some(provider) = acct.providers.first() else {
            return;
        };
        // Read before the pass: a change that settles leaves the queue, and a refused one still
        // has to be named on the pane.
        let folder_changes = self.queued_folder_changes(account).await;
        let acct = self.account_ordinal(account).await;
        self.log_due_sends(account, acct).await;
        match self.engine.drain_outbox(provider, account).await {
            Ok(report) if report.is_idle() => {}
            Ok(report) => {
                let after = self.engine.outbox(account).await.unwrap_or_default();
                log_drain_report(acct, &report, &after);
                // A queued draft save that got through resolved to a key, and this report is
                // the only place it is ever named (`docs/drafts.md`).
                self.record_drained_drafts(&report);
                self.settle_drained_folder_changes(provider, account, &report, &folder_changes)
                    .await;
                // A send that went out leaves the Outbox, and one that failed again carries a
                // new attempt count. Both are in the store now, so they are published before
                // the sync below, which a silent server can hold indefinitely. After the folder
                // settle rather than before it: a drained rename is out of the queue but not yet
                // in the stored tree until that settle re-reads it.
                if report
                    .attempted
                    .iter()
                    .any(|op| op.kind == PendingOpKind::MailSubmit)
                {
                    self.rebuild_snapshot().await;
                }
                self.refresh_after_write(account).await;
            }
            Err(err) => log::warn!("outbox[a{acct}]: a drain pass failed: {err}"),
        }
    }

    /// Logs each queued send the drain pass about to run will retry, before it runs, so a
    /// retry that never comes back still left a line saying it started.
    ///
    /// Which op is due is the engine's decision: this reads the same two facts it does (the op
    /// is waiting, and its backoff has elapsed) for a log line, never to decide anything. A
    /// send serialised behind another write on the same resource is named here and then left
    /// for a later pass, which the summary line after the pass shows.
    async fn log_due_sends(&self, account: &AccountId, acct: usize) {
        let Ok(queued) = self.engine.outbox(account).await else {
            return;
        };
        let Ok(now) = crate::helpers::now_utc() else {
            return;
        };
        for row in queued.iter().filter(|row| {
            row.kind == Some(PendingOpKind::MailSubmit)
                && row.state == PendingOpState::Pending
                && row.next_attempt_at.is_none_or(|due| due <= now)
        }) {
            log::info!(
                "outbox[a{acct}]: retrying queued send {}, attempt {}",
                row.id.get(),
                row.attempts.saturating_add(1)
            );
        }
    }

    /// The message waiting to be opened in the host's composer, if any.
    ///
    /// A lease-free pull, like every other standing question: the host reads it on a
    /// [`Surface::ComposeRequest`] signal and on relaunch.
    #[must_use]
    pub fn compose_request(&self) -> Option<ComposeRequest> {
        self.compose_request
            .lock()
            .expect("compose-request mutex poisoned")
            .clone()
    }

    /// Clears the request once the host's composer holds the message.
    ///
    /// Not a cancellation: the message left the outbox before the request existed, so
    /// there is nothing here to put back. Left unanswered it would reappear at every
    /// relaunch, offering to compose a message the user already has open.
    pub(crate) fn dismiss_compose_request(&self) {
        *self
            .compose_request
            .lock()
            .expect("compose-request mutex poisoned") = None;
        self.observer.surface_changed(Surface::ComposeRequest);
    }

    /// Hands the withdrawn message to the host's composer, as a standing request.
    ///
    /// The outbox no longer holds it at this point, so this is the only copy: it stays put
    /// until the host says the composer has it (`Intent::DismissComposeRequest`).
    fn raise_compose_request(&self, account: &AccountId, draft: &engine_api::Draft) {
        let request = ComposeRequest {
            account: account.as_str().to_owned(),
            to: join(&draft.to),
            cc: join(&draft.cc),
            bcc: join(&draft.bcc),
            subject: draft.subject.clone(),
            body_text: draft.text_body.clone(),
        };
        *self
            .compose_request
            .lock()
            .expect("compose-request mutex poisoned") = Some(request);
        self.observer.surface_changed(Surface::ComposeRequest);
    }

    /// The draft behind a queued send, or `None` when that op is not a send this build can
    /// read (or has already gone).
    async fn queued_draft(&self, queued: &QueuedRef) -> Option<engine_api::Draft> {
        let rows = self.engine.outbox(&queued.account).await.ok()?;
        rows.iter()
            .find(|row| row.id == queued.op)
            .and_then(engine_api::queued_draft)
    }
}

/// A recipient field as the composer takes it: one comma-joined string.
fn join(addresses: &[engine_api::EmailAddress]) -> String {
    addresses
        .iter()
        .map(|a| a.email.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
