//! Reporting a message to its provider: the mechanism behind mark-as-spam and its undo.
//!
//! A report is a **different verb** from the move it used to be. Filing a message under Junk
//! tells one mailbox where it lives; reporting it tells the provider something about the
//! message, which is what trains the filter that decides where the *next* one lands. Every
//! transport files the message as part of the report; to Junk for a junk or phishing verdict,
//! back to the Inbox for not-junk: so the report replaces the move rather than joining it.
//!
//! What the engine can promise differs per transport and is read from the capability, never
//! assumed: Gmail has no phishing verdict at all, and only Graph answers whether a report was
//! accepted. An account whose provider cannot report falls back to the plain move, which is
//! what the dev fixtures and the showcase engine do.

use engine_api::{MailboxRole, MessageReport, Provider, ReportVerdict};

use super::{MailWrite, WriteRefresh, folders::resolve_move_target, result::MailActionError};
use crate::{App, helpers::generated_idempotency, reference::MessageRef};

impl<P: Provider> App<P> {
    /// Reports `message` to its account's provider with `verdict`, filing it where the verdict
    /// says it belongs; Junk for junk and phishing, the Inbox for not-junk. The row leaves the
    /// list optimistically, exactly as a move's would.
    ///
    /// Falls back to filing the message ourselves when the account's provider advertises no
    /// reporting, or none for this verdict: the user asked for the message to be moved, and a
    /// provider that cannot be told is no reason to leave it where it is.
    ///
    /// Returns [`MailActionError::NoTargetFolder`] when the destination does not resolve and
    /// [`MailActionError::Rejected`] when the provider refused.
    pub(crate) async fn report(
        &self,
        message: MessageRef,
        verdict: ReportVerdict,
        refresh: WriteRefresh,
    ) -> Result<(), MailActionError> {
        let role = if verdict.files_as_junk() {
            MailboxRole::Junk
        } else {
            MailboxRole::Inbox
        };
        if !self.can_report(&message.account, verdict).await {
            return self.move_to_role(message, role, refresh).await;
        }
        // The destination the caller would have moved it to. The transports that file it
        // server-side ignore this; the ones that move it themselves need it, and neither
        // lets the caller ask for anywhere else.
        let mailboxes = self
            .engine
            .mailboxes(&message.account)
            .await
            .unwrap_or_default();
        let Some(destination) = resolve_move_target(&mailboxes, &role) else {
            log::warn!(
                "report: account {} has no {role:?} folder (no SPECIAL-USE role and no \
                 conventional name); skipping",
                message.account.as_str(),
            );
            return Err(MailActionError::NoTargetFolder);
        };
        let report = MessageReport::new(message.key.clone(), verdict, destination.id.clone());
        if self
            .remove_optimistically(message, MailWrite::Report(report), refresh)
            .await
        {
            Ok(())
        } else {
            Err(MailActionError::Rejected)
        }
    }

    /// Sends `report` to the account's provider without touching the list: the report twin of
    /// `edit_only`, including its re-consent handling: a report that lands proves the grant
    /// still carries the mail-write scope, and a refusal raises the same prompt an edit's
    /// would.
    pub(super) async fn report_only(
        &self,
        account: &engine_api::AccountId,
        report: &MessageReport,
    ) -> bool {
        // Clone the account handle, then report with the read guard released: the round-trip
        // must not hold the lock.
        if let Some(acct) = self.account_handle(account).await
            && let Some(provider) = acct.providers.first()
        {
            return match self
                .engine
                .report_message(provider, account, &generated_idempotency(), report)
                .await
            {
                Ok(_) => {
                    self.clear_mail_reauth_required(account);
                    true
                }
                Err(err) => {
                    // A class + protocol detail, never the message or its addresses.
                    log::warn!("report: mail report failed: {err}");
                    self.note_mail_write_error(account, &err);
                    false
                }
            };
        }
        false
    }

    /// Whether this account's first mail provider can express `verdict`.
    ///
    /// Read from [`Capabilities::mail_report`](engine_api::Capabilities::mail_report) rather
    /// than assumed, because the transports genuinely differ; Gmail's label set has no
    /// phishing member, so asking for that verdict is a hard error there rather than a
    /// near-enough filing under spam.
    async fn can_report(&self, account: &engine_api::AccountId, verdict: ReportVerdict) -> bool {
        self.account_handle(account)
            .await
            .and_then(|handle| {
                handle.providers.first().map(|provider| {
                    provider
                        .connection_info()
                        .capabilities
                        .mail_report()
                        .is_some_and(|controls| controls.verdicts.allows(verdict))
                })
            })
            .unwrap_or(false)
    }
}
