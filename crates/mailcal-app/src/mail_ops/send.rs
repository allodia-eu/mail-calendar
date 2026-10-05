//! Submitting a built [`Draft`] through the durable outbox and driving the send-status hint.
//! Split out of `mail_ops` (its parent, which keeps the compose/account helpers and the
//! mail-mutation actions) to stay under the 500-line limit; the `impl App` block here is a
//! continuation of that one.

use std::sync::atomic::Ordering;

use engine_api::{AccountId, Draft, PendingOpState, Provider};

use super::AUTO_CLEAR_DELAY;
use crate::{App, CompositionId, SendStatus};

/// How a submission ended, in the three states a caller can act on differently.
///
/// The middle one is the reason this is not a `bool`. A message can go out and still leave
/// the sender without a copy of it, and the two facts have to travel together: collapsing
/// them into "sent" loses the copy silently, and collapsing them into "failed" invites a
/// re-send of mail the recipients already have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendOutcome {
    /// Delivered, and the copy is in the account's Sent folder.
    Sent,
    /// Delivered, but the copy could not be filed in Sent.
    SentNotFiled,
    /// Not delivered **yet**: the attempt failed for a reason worth retrying, so the message
    /// is in the Outbox and will go out when the network comes back.
    ///
    /// Distinct from [`Failed`](Self::Failed) because the message is not lost: telling
    /// someone it failed invites them to write it again, and then both arrive.
    Queued,
    /// The message may have been delivered: it reached the point where the server could act
    /// on it, and no answer came back. It is in the Outbox, never retried on its own, and
    /// waits for the user or for its copy to appear in Sent.
    Unconfirmed,
    /// Not delivered, and nothing will retry it: the server refused it. The Outbox keeps it
    /// until the user sends it again, edits it or discards it.
    NotSent,
    /// Not delivered, and never reached the Outbox: nothing holds it but the composer's draft.
    Failed,
}

impl SendOutcome {
    /// Whether the message reached its recipients; true for both delivered outcomes.
    fn went_out(self) -> bool {
        matches!(self, Self::Sent | Self::SentNotFiled)
    }
}

impl<P: Provider> App<P> {
    /// Submits a built draft from `account` through the durable outbox, surfacing the send
    /// status as a `Sending` → terminal hint (a host's "sending…" → "sent" UI), then
    /// refreshes so the filed Sent copy appears. A failure is surfaced as the `Failed` status
    /// **and** logged with its provider/outbox error, so a failed send leaves a diagnostic
    /// trail rather than only a host-visible hint; e.g. a Graph `403 ErrorAccessDenied` when
    /// the OAuth grant lacks `Mail.Send`. The logged error is a class + protocol detail, never
    /// draft content or addresses.
    pub(crate) async fn send_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        composition: Option<&CompositionId>,
    ) {
        let _ = self.send_draft_result(account, draft, composition).await;
    }

    /// The body of [`send_draft`](Self::send_draft), returning **whether the draft went out**.
    /// Split so the agent adapter can report a failed send to its caller instead of leaving the
    /// outcome only in the host-visible `Failed` hint (which an assistant cannot see). The
    /// interactive path wraps it and discards the bool, so its behaviour is byte-identical.
    pub(crate) async fn send_draft_result(
        &self,
        account: &AccountId,
        draft: &Draft,
        composition: Option<&CompositionId>,
    ) -> bool {
        self.set_send_status(SendStatus::Sending);
        let outcome = self.submit_through_outbox(account, draft).await;
        let generation = self.set_send_status(match outcome {
            SendOutcome::Sent => SendStatus::Sent,
            SendOutcome::SentNotFiled => SendStatus::SentNotFiled,
            SendOutcome::Queued => SendStatus::Queued,
            SendOutcome::Unconfirmed => SendStatus::Unconfirmed,
            SendOutcome::NotSent => SendStatus::NotSent,
            SendOutcome::Failed => SendStatus::Failed,
        });
        // Every outcome may have changed the Outbox: a queued send joined it, and a rebuild
        // that ran during the attempt may have drawn a row that has since gone. The queue is in
        // the store, so it is published now rather than by the sync below, which a server that
        // accepts the connection and then says nothing can hold for as long as it likes
        // (`docs/sending.md`). The draft discard after this reaches the server too.
        self.rebuild_snapshot().await;
        // The send is what finishes with the composition, not the host.
        //
        // Once the Outbox holds the message, whether it went, is waiting, or was refused, that
        // is its one place, and the draft beside it is a second copy the user would find in
        // Drafts long after (`docs/drafts.md`). Only a send that never reached the Outbox keeps
        // it: the composer is gone by then and those words are nowhere else.
        //
        // Either way the record goes. A host dismisses the composer the moment the submit is
        // accepted, which is long before the message has been anywhere, so it cannot be the one
        // to forget the composition: its `Close` and this run as separate tasks, and a `Close`
        // landing first would leave this with no record to find and the draft in Drafts for
        // ever.
        if let Some(composition) = composition {
            if outcome == SendOutcome::Failed {
                self.close_composition(composition);
            } else {
                self.discard_draft(composition).await;
            }
        }
        self.refresh_after_write(account).await;
        self.clear_send_status_after_delay(generation).await;
        outcome.went_out()
    }

    /// Submits `draft` through `account`'s first provider (the outbox owns durability) and
    /// returns whether it was sent: the body of [`send_draft`](Self::send_draft) split out so
    /// the account read guard is released before the network round-trip (the SMTP/Graph call
    /// must not hold the lock). Any failure is **logged**: a provider/outbox error or a missing
    /// provider: so a failed send leaves a diagnostic trail, not only the host-visible `Failed`
    /// hint. A Graph `403 ErrorAccessDenied` (the grant lacks `Mail.Send`) additionally raises the
    /// account's mail re-consent prompt; a successful send clears it.
    async fn submit_through_outbox(&self, account: &AccountId, draft: &Draft) -> SendOutcome {
        let Some(handle) = self.account_handle(account).await else {
            log::warn!("send: no connected account to submit from; nothing was queued");
            return SendOutcome::Failed;
        };
        let Some(provider) = handle.providers.first() else {
            log::warn!("send: account has no provider to submit through; nothing was queued");
            return SendOutcome::Failed;
        };
        // Before the round trip, so a send that never comes back still left a line saying it
        // started.
        let acct = self.account_ordinal(account).await;
        log::info!("send[a{acct}]: submitting a message");
        match self.engine.submit_mail(provider, account, draft).await {
            Ok(outcome) => {
                // The send went out, so the grant carries `Mail.Send`; clear any standing
                // "reconnect to send" prompt for this account.
                self.clear_mail_reauth_required(account);
                match outcome.sent_copy.unfiled_detail() {
                    None => {
                        log::info!("send[a{acct}]: delivered, and the copy is in Sent");
                        SendOutcome::Sent
                    }
                    Some(detail) => {
                        // The mail has reached its recipients and the sender's copy is not in
                        // Sent, and will not appear later, because there is nothing on the
                        // server for a sync to find. Raise the standing question so the user
                        // can file it, and log it either way.
                        log::warn!(
                            "send[a{acct}]: delivered, but the Sent copy was not filed: {detail}"
                        );
                        self.note_unfiled_copy(account, draft, detail);
                        SendOutcome::SentNotFiled
                    }
                }
            }
            Err(err) => {
                // The engine decides what became of a failed attempt and records it on the op,
                // so the queue itself answers "is this message lost?": asked of the store
                // rather than re-derived from the error's class here, which would be a second
                // copy of a rule the engine already owns.
                match self.queued_state(account, draft).await {
                    Some((op, PendingOpState::Pending | PendingOpState::InFlight)) => {
                        log::info!(
                            "send[a{acct}]: not sent yet; it is in the Outbox as queued send \
                             {op}: {err}"
                        );
                        SendOutcome::Queued
                    }
                    Some((op, PendingOpState::NeedsConfirmation)) => {
                        log::warn!(
                            "send[a{acct}]: the server may have accepted the message but did not \
                             confirm it; it is in the Outbox as queued send {op}, waiting for an \
                             answer, and will not be sent again on its own: {err}"
                        );
                        SendOutcome::Unconfirmed
                    }
                    Some((op, _)) => {
                        log::warn!(
                            "send[a{acct}]: not sent; it stays in the Outbox as queued send {op} \
                             until it is sent again, edited or discarded: {err}"
                        );
                        self.note_mail_write_error(account, &err);
                        SendOutcome::NotSent
                    }
                    None => {
                        log::error!(
                            "send[a{acct}]: not sent, and the message was not kept in the \
                             Outbox: {err}"
                        );
                        self.note_mail_write_error(account, &err);
                        SendOutcome::Failed
                    }
                }
            }
        }
    }

    /// Where `draft` stands in `account`'s outbox, with its number in the queue, or `None`
    /// when the outbox does not hold it.
    ///
    /// Matched on the `Message-ID` the draft carries, which is what the engine makes the op
    /// idempotent by, so a second send of the same draft finds the same row.
    async fn queued_state(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> Option<(u64, PendingOpState)> {
        let queued = self.engine.outbox(account).await.ok()?;
        queued.iter().find_map(|row| {
            engine_api::queued_draft(row)
                .filter(|d| d.message_id == draft.message_id)
                .map(|_| (row.id.get(), row.state))
        })
    }

    /// Surfaces a rich-draft build failure as a failed send. The composer document has
    /// already validated at the FFI boundary, so a build failure here is an unexpected
    /// attachment/header mismatch; shown as a `Failed` hint (which auto-clears) rather
    /// than a silent no-op, since the host has already dismissed the composer.
    pub(crate) async fn fail_send(&self) {
        let generation = self.set_send_status(SendStatus::Failed);
        self.clear_send_status_after_delay(generation).await;
    }

    /// Auto-clears a terminal send status back to [`SendStatus::Idle`] after
    /// [`AUTO_CLEAR_DELAY`]: the single place the "sent"/"failed" hint expires, so every
    /// client just renders `send_status()` on each [`crate::Surface::Sending`] signal. The
    /// reset is **guarded** by the captured `generation`: if a newer send changed the status
    /// during the delay, this older timer is stale and does nothing: the newer send wins.
    ///
    /// Awaited in-line at the tail of the caller (e.g. [`App::send_draft`]) rather than
    /// spawned: the app holds no runtime handle of its own (it runs on the bindings'
    /// runtime), and `dispatch` already runs each intent in a fire-and-forget task there, so
    /// the delay simply keeps that one task alive: no extra spawn, no `Arc<Self>` plumbing.
    async fn clear_send_status_after_delay(&self, generation: u64) {
        tokio::time::sleep(AUTO_CLEAR_DELAY).await;
        if self.send_status_generation.load(Ordering::SeqCst) == generation {
            self.set_send_status(SendStatus::Idle);
        }
    }
}
