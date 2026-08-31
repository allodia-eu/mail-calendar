//! The **result-returning** mail actions: the same writes the UI dispatches, reported back to
//! their caller instead of fired and forgotten.
//!
//! An [`Intent`](crate::Intent) is deliberately fire-and-forget: the interactive surface learns
//! what happened from the optimistic hide being undone and the list re-rendering, which is the
//! right shape for a person watching their own mailbox. An **agent adapter** has no list to
//! watch. It has to answer "did that work?", and a silent no-op reported as success is the worst
//! possible answer: `crates/mailcal-account/src/google.rs` wraps Gmail without forwarding
//! `edit_mail`, so a mark-read on a Google account applies nothing at all; with no result to
//! carry, an assistant would cheerfully report that it had marked the message read.
//!
//! So these methods take the **same code path** to the provider as the interactive handlers (the
//! same optimistic hide, the same edit, the same rejection handling in `mail_ops`), differing in
//! exactly two ways:
//!
//! 1. They pre-check the account, the message, and the provider, so
//!    [`MailActionError::UnknownAccount`] / [`MailActionError::UnknownMessage`] /
//!    [`MailActionError::NoProvider`] are distinguishable: a bare `false` cannot express which.
//! 2. They **defer** the account-wide re-sync and throttle it to one per [`COALESCE_WINDOW`]. Fifty
//!    scripted archives must not become fifty full syncs against the user's own server.

use std::time::{Duration, Instant};

use engine_api::{AccountId, EmailAddress, MailEdit, MailboxRole, Provider, ReportVerdict};

use super::WriteRefresh;
use crate::{App, reference::MessageRef};

/// The shortest interval between two account-wide re-syncs driven by the **agent** write path.
///
/// A write inside the window still applies; it is only the follow-up *sync* that is throttled,
/// and the list is re-projected locally instead. So a scripted "archive these fifty" costs one
/// sync rather than fifty, while a lone action still syncs immediately and adds no latency.
const COALESCE_WINDOW: Duration = Duration::from_secs(2);

/// Why a mail action did not happen.
///
/// A **closed enum**, never a message: user-facing strings live in the clients' localization
/// catalogs, and this crosses into an adapter rather than a UI. A caller that needs to explain
/// the failure to a person maps the variant itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailActionError {
    /// No account with that id is configured.
    UnknownAccount,
    /// The account is configured, but holds no synced message with that provider key.
    UnknownMessage,
    /// The account has no connected mail provider; still dialing, an outage, or a provider
    /// family that does not implement mail edits at all (Gmail today).
    NoProvider,
    /// The action needed a destination folder (Archive, Trash, Junk, Inbox) and the account
    /// advertises none, under either its RFC 6154 role or a conventional name.
    NoTargetFolder,
    /// The provider was asked and refused: a revoked scope, a stale key, a server error.
    Rejected,
}

/// Why a direct send did not happen. Separate from [`MailActionError`] because a send fails in
/// its own ways (no recipient, an unbuildable draft) that a mailbox mutation cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendActionError {
    /// No account with that id is configured, or none is configured at all.
    UnknownAccount,
    /// Every recipient list was empty. A recipient-less draft is never sent: the command layer
    /// enforces this for every caller, not just the clients' Send-button gating.
    NoRecipients,
    /// The draft could not be assembled (a malformed generated `Message-ID`).
    DraftFailed,
    /// The submission itself failed: no provider, a refused scope, or an SMTP/API error.
    Rejected,
}

impl<P: Provider> App<P> {
    /// Marks `message` read or unread and reports the outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_mark_read(
        &self,
        message: &MessageRef,
        read: bool,
    ) -> Result<(), MailActionError> {
        self.act_keyworded(message, |target| MailEdit::mark_seen(target, read))
            .await
    }

    /// Flags or unflags `message` and reports the outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_set_flagged(
        &self,
        message: &MessageRef,
        flagged: bool,
    ) -> Result<(), MailActionError> {
        self.act_keyworded(message, |target| MailEdit::set_flagged(target, flagged))
            .await
    }

    /// Archives `message` into its account's Archive folder and reports the outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_archive(&self, message: &MessageRef) -> Result<(), MailActionError> {
        self.act_move(message, MailboxRole::Archive).await
    }

    /// Moves `message` to its account's Trash folder (recoverable) and reports the outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_trash(&self, message: &MessageRef) -> Result<(), MailActionError> {
        self.act_move(message, MailboxRole::Trash).await
    }

    /// Reports `message` to its account's provider as junk (which files it under Junk) and
    /// reports the outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_spam(&self, message: &MessageRef) -> Result<(), MailActionError> {
        self.act_report(message, ReportVerdict::Junk).await
    }

    /// Reports `message` as not junk (which files it back in the Inbox) and reports the
    /// outcome.
    ///
    /// # Errors
    ///
    /// See [`MailActionError`].
    pub async fn act_not_spam(&self, message: &MessageRef) -> Result<(), MailActionError> {
        self.act_report(message, ReportVerdict::NotJunk).await
    }

    /// Sends a plain-text message and reports the outcome: the direct-send path an agent
    /// adapter uses when the user has explicitly turned direct sending on.
    ///
    /// `from` names the sending account; `None` derives it exactly as the composer does
    /// (`App::compose_account`). The recipient lists are already-split addresses; at least one
    /// of the three must be non-empty. The body is plain text with no HTML alternative; an
    /// assistant composes prose, and an HTML body it authored is a rendering surface nobody
    /// asked for.
    ///
    /// # Errors
    ///
    /// See [`SendActionError`].
    pub async fn act_send_plain(
        &self,
        from: Option<&AccountId>,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: String,
        body: String,
    ) -> Result<(), SendActionError> {
        let account = match from {
            Some(account) => account.clone(),
            None => self
                .compose_account()
                .await
                .ok_or(SendActionError::UnknownAccount)?,
        };
        let identity = self
            .identity_or_fail(&account)
            .await
            .ok_or(SendActionError::UnknownAccount)?;
        let (to, cc, bcc) = (addresses(to), addresses(cc), addresses(bcc));
        if to.is_empty() && cc.is_empty() && bcc.is_empty() {
            self.fail_send().await;
            return Err(SendActionError::NoRecipients);
        }
        let draft = super::plain_draft(&identity, to, subject, body)
            .ok_or(SendActionError::DraftFailed)?
            .with_cc(cc)
            .with_bcc(bcc);
        if self.send_draft_result(&account, &draft).await {
            Ok(())
        } else {
            Err(SendActionError::Rejected)
        }
    }

    /// The shared shape of the in-place edits (mark-read, flag): pre-check, apply through the
    /// same helper the interactive handler uses, then one coalesced refresh.
    async fn act_keyworded(
        &self,
        message: &MessageRef,
        build: impl FnOnce(engine_api::ProviderKey) -> MailEdit,
    ) -> Result<(), MailActionError> {
        self.precheck(message).await?;
        let applied = self
            .edit_keyworded(message.clone(), WriteRefresh::Deferred, build)
            .await;
        self.settle_after_write(&message.account).await;
        if applied {
            Ok(())
        } else {
            Err(MailActionError::Rejected)
        }
    }

    /// The shared shape of the two reports (spam, not-spam): the report twin of
    /// [`act_move`](Self::act_move), deferring its re-sync the same way.
    async fn act_report(
        &self,
        message: &MessageRef,
        verdict: ReportVerdict,
    ) -> Result<(), MailActionError> {
        self.precheck(message).await?;
        let outcome = self
            .report(message.clone(), verdict, WriteRefresh::Deferred)
            .await;
        self.settle_after_write(&message.account).await;
        outcome
    }

    /// The shared shape of the folder moves (archive, trash).
    async fn act_move(
        &self,
        message: &MessageRef,
        role: MailboxRole,
    ) -> Result<(), MailActionError> {
        self.precheck(message).await?;
        let outcome = self
            .move_to_role(message.clone(), role, WriteRefresh::Deferred)
            .await;
        self.settle_after_write(&message.account).await;
        outcome
    }

    /// Resolves what a bare `false` cannot express: is the account configured, does it hold this
    /// message, and does it have a provider to write through? Runs before every action so an
    /// assistant is told *why* nothing happened rather than only *that* nothing did.
    async fn precheck(&self, message: &MessageRef) -> Result<(), MailActionError> {
        let account = self
            .account_handle(&message.account)
            .await
            .ok_or(MailActionError::UnknownAccount)?;
        if account.providers.is_empty() {
            return Err(MailActionError::NoProvider);
        }
        if self.find_message_in(message).await.is_none() {
            return Err(MailActionError::UnknownMessage);
        }
        Ok(())
    }

    /// Settles the list after a deferred write, driving **at most one account-wide re-sync per
    /// [`COALESCE_WINDOW`]**.
    ///
    /// A write's follow-up syncs the one account it reached. On the interactive path that is the
    /// whole cost; one swipe, one refresh, and a person cannot swipe fast enough to matter. An
    /// agent can: fifty scripted archives would be fifty syncs against the user's own server,
    /// which is a denial of service the user paid for. So a write inside the window re-projects
    /// the list from what is already in memory (`rebuild_snapshot`, no network) instead.
    ///
    /// Nothing is lost by skipping the sync. The row is already hidden optimistically, the edit
    /// has already reached the provider (that is what the tool result reports), and the next
    /// leading-edge sync (or the account's standing IDLE watch / poll timer) reconciles the
    /// server's view. A debounce would be the wrong instrument here: agent tool calls arrive one
    /// at a time, each awaited, so a "wait and see if another follows" window would simply add
    /// latency to every call and still sync once per write.
    ///
    /// Deliberately **not** a rate limit on the write itself. Throttling the *action* would make
    /// an assistant's archive silently not happen; throttling only the follow-up sync costs
    /// nothing a user can observe.
    async fn settle_after_write(&self, account: &AccountId) {
        let due = {
            let mut last = self
                .write_refresh_at
                .lock()
                .expect("agent-refresh mutex poisoned");
            let due = last.is_none_or(|at| at.elapsed() >= COALESCE_WINDOW);
            if due {
                *last = Some(Instant::now());
            }
            due
        };
        if due {
            self.refresh_after_write(account).await;
        } else {
            self.rebuild_snapshot().await;
        }
    }
}

/// Turns already-split recipient strings into addresses, dropping blanks.
fn addresses(list: &[String]) -> Vec<EmailAddress> {
    list.iter()
        .map(|address| address.trim())
        .filter(|address| !address.is_empty())
        .map(EmailAddress::new)
        .collect()
}
