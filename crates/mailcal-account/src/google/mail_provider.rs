//! The [`Provider`] trait implementation for `RefreshingGmailProvider`; the
//! token-refreshing, 429-backoff, and stale-socket reconnect loops wrapped around each
//! delegated Gmail call, plus the two write paths (`edit_mail`, `submit_email`). Split from
//! the module root (`super`), which holds the struct, its delegate cache, and the connect
//! entry points, to keep both files under the 500-line cap; mirroring
//! [`crate::graph`]'s identical split.

use async_trait::async_trait;
use engine_core::{
    ids::AccountId,
    mail::{Mailbox, Message},
    raw::RawMime,
    sync::{SyncScope, SyncState, SyncWindow},
};
use engine_provider::{
    ConnectionInfo, Draft, EmailStream, MailEdit, MailEditReceipt, MessageReport, Provider,
    ProviderResult, ReportReceipt, ScopeSync, SubmissionReceipt,
};
use futures::StreamExt;

use super::{RefreshingGmailProvider, should_reconnect};

#[async_trait]
impl Provider for RefreshingGmailProvider {
    fn connection_info(&self) -> ConnectionInfo {
        // Capped capabilities, the delegate's transport facts; see `delegate_info`.
        crate::delegate_info::with_delegate_transport(
            ConnectionInfo::new(self.capabilities),
            self.cached
                .lock()
                .expect("gmail delegate mutex poisoned")
                .as_ref()
                .map(|(_, provider)| provider.connection_info()),
        )
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GmailLabelList {
            account: account.clone(),
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GmailMessages {
            account: account.clone(),
        }
    }

    async fn sync_mailboxes(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.sync_mailboxes(account, cursor).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    fn stream_email<'a>(
        &'a self,
        account: &'a AccountId,
        cursor: Option<&'a SyncState>,
        window: SyncWindow,
        fetch_batch: usize,
        chunk_size: usize,
    ) -> EmailStream<'a> {
        let account = account.clone();
        let cursor = cursor.cloned();
        Box::pin(async_stream::try_stream! {
            let mut reconnected = false;
            loop {
                let provider = self.delegate().await?;
                let mut chunks = Vec::new();
                let result = {
                    let mut stream = provider.stream_email(
                        &account,
                        cursor.as_ref(),
                        window,
                        fetch_batch,
                        chunk_size,
                    );
                    let mut result = Ok(());
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(chunk) => chunks.push(chunk),
                            Err(err) => {
                                result = Err(err);
                                break;
                            }
                        }
                    }
                    result
                };
                match result {
                    Ok(()) => {
                        for chunk in chunks {
                            yield chunk;
                        }
                        break;
                    }
                    Err(err) if !reconnected && should_reconnect(&err) => {
                        self.invalidate_delegate();
                        reconnected = true;
                    }
                    Err(err) => Err(err)?,
                }
            }
        })
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.fetch_message_source(account, message).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Applies a [`MailEdit`] (mark-read/flag, move/archive, permanent delete) by delegating
    /// to the underlying `GmailProvider`, with the same token-refresh, 429 backoff, and
    /// stale-socket re-dial as the read calls. Gmail's edits are keyed by the message's
    /// immutable Gmail id, and this provider is account-global, so it can edit a message
    /// under any label.
    ///
    /// Safe to wrap like the idempotent reads: a Gmail edit is a label **delta** to a fixed
    /// state (`modify`/`trash`/`delete`), so re-applying it lands on the same result; see
    /// [`should_reconnect`].
    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.edit_mail(account, edit).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Submits a [`Draft`] via `messages.send` by delegating to the underlying
    /// `GmailProvider` (Gmail files the Sent copy itself), refreshing the access token first
    /// and backing off on a 429; but, **unlike** the reads and [`edit_mail`](Self::edit_mail),
    /// it **never re-issues the send on a transport failure**.
    ///
    /// A send is the one non-idempotent Gmail call: re-`POST`ing `messages.send` delivers the
    /// message twice. A retryable transport error covers both a request that died at the
    /// socket *before* Gmail accepted it (a resend would be safe) and one lost *after* Gmail
    /// queued it (a resend double-sends); indistinguishable here. The engine's outbox is
    /// built on exactly that: it calls `submit_email` once and never blind-retries, parking an
    /// ambiguous loss for the user to confirm rather than risk a duplicate. So on a transport
    /// error this only **drops** the (possibly dead-socketed) cached delegate, so the user's
    /// *deliberate* retry dials a fresh connection, then propagates the error.
    ///
    /// A throttled send *is* re-issued, one layer down: Google rejects it *before* acting on
    /// it, so the message never left and a replay cannot double-deliver. Mirrors the identical
    /// rule in [`crate::graph`]'s wrapper.
    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        let provider = self.delegate().await?;
        provider
            .submit_email(account, draft)
            .await
            .inspect_err(|err| {
                // A transport failure may have been a send that *did* leave; drop the suspect
                // socket for the next deliberate attempt, but never resend here.
                if should_reconnect(err) {
                    self.invalidate_delegate();
                }
            })
    }

    /// Gmail reports a message by moving it under its `SPAM` label, which this forwards on the
    /// same reconnect loop [`edit_mail`](Provider::edit_mail) uses: a report is idempotent, so a
    /// retry after a stale socket cannot double-apply.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            match provider.report_message(account, report).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }
}
