//! The standing question a client asks when a message went out but its copy never reached
//! the account's Sent folder, and the repair that answers it.
//!
//! This is deliberately louder than the transient send hint beside it. A Sent copy is how a
//! person checks that a message really left, so losing one silently is worse than most
//! failures that *do* interrupt: the mail is with its recipients, and the only record the
//! sender has of it is gone. Nothing later recovers it either; there is no copy on the
//! server for a sync to find: so the moment it happens is the only moment to say so.
//!
//! What the repair is **not** is a re-send. Delivering and filing cannot be one transaction:
//! the only way to be sure a copy exists would be to re-run the send, and re-running a send
//! that already succeeded puts the message in front of its recipients twice. So the two stay
//! separate, the delivery is never repeated, and the filing alone is retried; safely,
//! because the provider probes for the copy before placing one.

use engine_api::{AccountId, Draft, Provider};

use crate::{App, Surface};

/// A message that was delivered but whose copy is not in Sent, and what a retry needs.
///
/// Held in memory for the session. A restart loses the chance to retry: the message stays
/// sent, and the copy stays missing, which is the one gap this leaves open; making it
/// durable means an outbox op recording a submission that already succeeded, and that is a
/// larger change than the loss justifies today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfiledCopy {
    /// The account whose Sent folder is missing the copy.
    pub account: AccountId,
    /// The message as it was sent, so the repair can file exactly what went out. The
    /// `Message-ID` on it is what makes the retry idempotent.
    pub draft: Draft,
    /// The sent message's subject, so the modal can name what is missing.
    pub subject: String,
    /// Why it could not be filed: a failure class and protocol detail for the diagnostics
    /// screen and the log, never addresses or body text.
    pub detail: String,
    /// Whether a retry is in flight, so a client can disable its button rather than let the
    /// user queue five of them.
    pub retrying: bool,
}

impl UnfiledCopy {
    /// The same question, no longer retrying and carrying `detail` if the attempt supplied a
    /// new reason; what a failed repair leaves standing for the user to press again.
    fn settled(&self, detail: Option<String>) -> Self {
        Self {
            detail: detail.unwrap_or_else(|| self.detail.clone()),
            retrying: false,
            ..self.clone()
        }
    }
}

impl<P: Provider> App<P> {
    /// The standing question, if a send has left a copy unfiled.
    #[must_use]
    pub fn unfiled_copy(&self) -> Option<UnfiledCopy> {
        self.unfiled_copy
            .lock()
            .expect("unfiled-copy mutex poisoned")
            .clone()
    }

    /// Raises or clears the question, signalling the surface either way.
    pub(crate) fn set_unfiled_copy(&self, unfiled: Option<UnfiledCopy>) {
        *self
            .unfiled_copy
            .lock()
            .expect("unfiled-copy mutex poisoned") = unfiled;
        self.observer.surface_changed(Surface::UnfiledCopy);
    }

    /// Records that `draft` went out on `account` but its copy was not filed.
    pub(crate) fn note_unfiled_copy(&self, account: &AccountId, draft: &Draft, detail: &str) {
        self.set_unfiled_copy(Some(UnfiledCopy {
            account: account.clone(),
            draft: draft.clone(),
            subject: draft.subject.clone(),
            detail: detail.to_owned(),
            retrying: false,
        }));
    }

    /// Dismisses the question without retrying: the user has read it and chosen to live
    /// with the missing copy.
    pub(super) fn dismiss_unfiled_copy(&self) {
        self.set_unfiled_copy(None);
    }

    /// Files the copy of the already-sent message, and returns whether it landed.
    ///
    /// Sends nothing: the message left when it was submitted, and this places the copy the
    /// first attempt could not. Safe to press repeatedly: the provider probes for the copy
    /// before placing one, but a retry already in flight is ignored rather than run twice.
    ///
    /// On success the question clears and the mailbox re-syncs, so the copy appears in Sent
    /// where the user was told to expect it. On failure the question stays, carrying the new
    /// reason, so the button can be pressed again.
    pub(super) async fn retry_unfiled_copy(&self) -> bool {
        let Some(pending) = self.begin_unfiled_retry() else {
            return false;
        };
        let Some(account) = self.account_handle(&pending.account).await else {
            log::warn!("sent copy: the account is no longer connected; cannot file it");
            self.finish_unfiled_retry(&pending, Some(pending.settled(None)));
            return false;
        };
        let Some(provider) = account.providers.first() else {
            log::warn!("sent copy: the account has no provider to file through");
            self.finish_unfiled_retry(&pending, Some(pending.settled(None)));
            return false;
        };
        match self
            .engine
            .file_sent_copy(provider, &pending.account, &pending.draft)
            .await
        {
            Ok(_) => {
                log::info!("sent copy: filed on retry");
                self.finish_unfiled_retry(&pending, None);
                // Re-sync so the copy the user was just promised actually shows up in Sent.
                self.refresh_after_write(&pending.account).await;
                true
            }
            Err(err) => {
                log::warn!("sent copy: still could not be filed: {err}");
                self.finish_unfiled_retry(&pending, Some(pending.settled(Some(format!("{err}")))));
                false
            }
        }
    }

    /// Writes a retry's outcome back, but **only while the question it started from is still
    /// the one standing**.
    ///
    /// A second send can fail to file while this one is in flight, and it replaces the
    /// question with its own. Writing this retry's result over that would either clear a
    /// question nobody has answered; losing the newer message's copy for good, since nothing
    /// later rediscovers one, or relabel it with the older message's subject and reason.
    fn finish_unfiled_retry(&self, started: &UnfiledCopy, outcome: Option<UnfiledCopy>) {
        let mut guard = self
            .unfiled_copy
            .lock()
            .expect("unfiled-copy mutex poisoned");
        // The `Message-ID` is minted per submission, so it is what tells the two apart.
        if guard
            .as_ref()
            .is_none_or(|current| current.draft.message_id != started.draft.message_id)
        {
            return;
        }
        *guard = outcome;
        drop(guard);
        self.observer.surface_changed(Surface::UnfiledCopy);
    }

    /// Marks the pending question as retrying and hands back what the retry needs, or `None`
    /// when there is nothing to retry or one is already running.
    fn begin_unfiled_retry(&self) -> Option<UnfiledCopy> {
        let mut guard = self
            .unfiled_copy
            .lock()
            .expect("unfiled-copy mutex poisoned");
        let pending = guard.as_ref()?;
        if pending.retrying {
            return None;
        }
        let started = UnfiledCopy {
            retrying: true,
            ..pending.clone()
        };
        *guard = Some(started.clone());
        drop(guard);
        self.observer.surface_changed(Surface::UnfiledCopy);
        Some(started)
    }
}
