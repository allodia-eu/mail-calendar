//! The Outbox: showing the queued sends, acting on one, and draining them when the network
//! comes back.
//!
//! The engine owns the queue and every decision about it (`store-and-sync.md`): what is due,
//! whether a failure parks or settles, and whether an op can be withdrawn at all. This layer
//! decides only what a *person* is offered, and turns each refusal into something the app can
//! say out loud rather than a silent no-op.

use engine_api::{AccountId, OpRejection, Provider};

use crate::{App, ComposeRequest, OutboxIntent, Scope, Surface, reference::QueuedRef};

impl<P: Provider> App<P> {
    /// Routes one [`OutboxIntent`] to its handler.
    pub(crate) async fn dispatch_outbox(&self, intent: OutboxIntent) {
        match intent {
            OutboxIntent::Show => self.show_outbox().await,
            OutboxIntent::Cancel(queued) => self.cancel_queued_send(&queued).await,
            OutboxIntent::SendNow(queued) => self.send_queued_now(&queued).await,
            OutboxIntent::Edit(queued) => self.edit_queued_send(&queued).await,
        }
    }

    /// Shows the Outbox: the queued sends across every account, in one list.
    pub(crate) async fn show_outbox(&self) {
        *self.scope.lock().expect("scope mutex poisoned") = Scope::Outbox;
        self.rebuild_snapshot().await;
    }

    /// Withdraws a queued send so it is never delivered.
    ///
    /// A refusal is **logged and left on screen**, never swallowed: the row the user pressed
    /// is still there afterwards, and the next rebuild shows why. The one refusal that
    /// matters is `InFlight`, and the honest reading of it is that the message may already
    /// have gone.
    pub(crate) async fn cancel_queued_send(&self, queued: &QueuedRef) {
        match self
            .engine
            .cancel_pending_op(&queued.account, queued.op)
            .await
        {
            Ok(None) => log::info!("outbox: a queued send was withdrawn before it went out"),
            Ok(Some(refusal)) => log_refusal("withdraw", refusal),
            Err(err) => log::warn!("outbox: withdrawing a queued send failed: {err}"),
        }
        self.rebuild_snapshot().await;
    }

    /// Attempts a queued send now instead of waiting out its backoff, then drains so the
    /// attempt actually happens in this gesture rather than at some later pass.
    pub(crate) async fn send_queued_now(&self, queued: &QueuedRef) {
        match self
            .engine
            .retry_pending_op_now(&queued.account, queued.op)
            .await
        {
            Ok(None) => self.drain_account(&queued.account).await,
            Ok(Some(refusal)) => log_refusal("send now", refusal),
            Err(err) => log::warn!("outbox: hurrying a queued send failed: {err}"),
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
        let Some(draft) = self.queued_draft(queued).await else {
            log::warn!("outbox: no queued send to edit under that id");
            return;
        };
        match self
            .engine
            .cancel_pending_op(&queued.account, queued.op)
            .await
        {
            Ok(None) => {}
            Ok(Some(refusal)) => {
                log_refusal("edit", refusal);
                self.rebuild_snapshot().await;
                return;
            }
            Err(err) => {
                log::warn!("outbox: withdrawing a queued send to edit it failed: {err}");
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
    pub(crate) async fn flush_outboxes(&self) {
        for account in self.account_ids().await {
            let queued = self.engine.outbox(&account).await.unwrap_or_default();
            for row in &queued {
                // A refusal here is ordinary (in flight, or awaiting confirmation) and the
                // drain below is unaffected by it, so it is not worth a line of its own.
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
        match self.engine.drain_outbox(provider, account).await {
            Ok(report) if report.is_idle() => {}
            Ok(report) => {
                log::info!(
                    "outbox: a drain pass attempted {} queued write(s), {} delivered",
                    report.attempted.len(),
                    report.delivered()
                );
                // A queued draft save that got through resolved to a key, and this report is
                // the only place it is ever named (`docs/drafts.md`).
                self.record_drained_drafts(&report);
                self.refresh_after_write(account).await;
            }
            Err(err) => log::warn!("outbox: a drain pass failed: {err}"),
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

/// Logs why a host action on a queued send did not take effect.
///
/// Every one of these is a state the user can see resolve itself: an in-flight send lands, a
/// settled one leaves the list. So the app says what happened and rebuilds, rather than
/// raising a question nobody can answer.
fn log_refusal(action: &str, refusal: OpRejection) {
    let reason = match refusal {
        OpRejection::Unknown => "it is no longer queued",
        OpRejection::Settled => "it has already finished",
        OpRejection::InFlight => "it is being sent right now",
        OpRejection::AwaitingConfirmation => "it may already have been delivered",
    };
    log::info!("outbox: could not {action} a queued send: {reason}");
}

/// A recipient field as the composer takes it: one comma-joined string.
fn join(addresses: &[engine_api::EmailAddress]) -> String {
    addresses
        .iter()
        .map(|a| a.email.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
