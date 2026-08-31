//! Submitting a built [`Draft`] through the durable outbox and driving the send-status hint.
//! Split out of `mail_ops` (its parent, which keeps the compose/account helpers and the
//! mail-mutation actions) to stay under the 500-line limit; the `impl App` block here is a
//! continuation of that one.

use std::sync::atomic::Ordering;

use engine_api::{AccountId, Draft, Provider};

use super::AUTO_CLEAR_DELAY;
use crate::{App, SendStatus};

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
    /// Not delivered.
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
    pub(crate) async fn send_draft(&self, account: &AccountId, draft: &Draft) {
        let _ = self.send_draft_result(account, draft).await;
    }

    /// The body of [`send_draft`](Self::send_draft), returning **whether the draft went out**.
    /// Split so the agent adapter can report a failed send to its caller instead of leaving the
    /// outcome only in the host-visible `Failed` hint (which an assistant cannot see). The
    /// interactive path wraps it and discards the bool, so its behaviour is byte-identical.
    pub(crate) async fn send_draft_result(&self, account: &AccountId, draft: &Draft) -> bool {
        self.set_send_status(SendStatus::Sending);
        let outcome = self.submit_through_outbox(account, draft).await;
        let generation = self.set_send_status(match outcome {
            SendOutcome::Sent => SendStatus::Sent,
            SendOutcome::SentNotFiled => SendStatus::SentNotFiled,
            SendOutcome::Failed => SendStatus::Failed,
        });
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
        let Some(acct) = self.account_handle(account).await else {
            log::warn!("send: no connected account to submit from");
            return SendOutcome::Failed;
        };
        let Some(provider) = acct.providers.first() else {
            log::warn!("send: account has no provider to submit through");
            return SendOutcome::Failed;
        };
        match self.engine.submit_mail(provider, account, draft).await {
            Ok(outcome) => {
                // The send went out, so the grant carries `Mail.Send`; clear any standing
                // "reconnect to send" prompt for this account.
                self.clear_mail_reauth_required(account);
                match outcome.sent_copy.unfiled_detail() {
                    None => SendOutcome::Sent,
                    Some(detail) => {
                        // The mail has reached its recipients and the sender's copy is not in
                        // Sent, and will not appear later, because there is nothing on the
                        // server for a sync to find. Raise the standing question so the user
                        // can file it, and log it either way.
                        log::warn!("send: delivered, but the Sent copy was not filed: {detail}");
                        self.note_unfiled_copy(account, draft, detail);
                        SendOutcome::SentNotFiled
                    }
                }
            }
            Err(err) => {
                log::warn!("send: submission failed: {err}");
                self.note_mail_write_error(account, &err);
                SendOutcome::Failed
            }
        }
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
