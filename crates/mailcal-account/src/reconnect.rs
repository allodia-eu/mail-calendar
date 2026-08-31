//! An IMAP [`Provider`] wrapper that transparently reconnects a dropped session.
//!
//! The engine's `ImapProvider` holds **one** persistent `Mutex<Connection>` and never
//! re-dials: once its TLS socket dies (the machine slept, or the network dropped), every
//! reuse fails instantly with a [`FailureClass::Retryable`] transport error (`Broken pipe`,
//! `peer closed connection without sending TLS close_notify`) and stays dead until the
//! provider is rebuilt. The app holds its providers behind an immutable `Arc`, so nothing
//! rebuilds them and only an app restart recovers, which is exactly the "Refresh does
//! nothing / can't load this message" bug.
//!
//! [`ReconnectingImapProvider`] fixes it the same way [`RefreshingGraphProvider`] fixes the
//! Graph side ([`crate::graph`]): it caches a live delegate and, on a retryable transport
//! failure, drops the dead session, re-dials a fresh one, and retries the (idempotent) call
//! once. The re-dial is an **injected closure** ([`Redial`]) so the reconnect path is
//! unit-testable without a real socket; the live closure ([`crate::make_imap_redial`])
//! re-runs `ImapProvider::connect`.
//!
//! [`RefreshingGraphProvider`]: crate::graph

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId},
    mail::{Mailbox, Message},
    raw::RawMime,
    sync::{SyncScope, SyncState, SyncWindow},
};
use engine_provider::{
    Capabilities, ConnectionInfo, Draft, EmailStream, MailEdit, MailEditReceipt, MessageReport,
    Provider, ProviderResult, ReportReceipt, ScopeSync, SubmissionReceipt,
};
use futures::StreamExt;

/// Re-dials a fresh, logged-in IMAP session bound to the wrapper's mailbox. Boxed so it can
/// be injected (a real `ImapProvider::connect` on the live path, a fake in tests).
pub(crate) type Redial = Box<
    dyn Fn() -> Pin<Box<dyn Future<Output = ProviderResult<Arc<dyn Provider>>> + Send>>
        + Send
        + Sync,
>;

/// Wraps an IMAP mail provider and reconnects it after a dropped connection. Bound to one
/// mailbox (its email scope), mirroring the engine's `ImapProvider`.
pub(crate) struct ReconnectingImapProvider {
    /// Rebuilds a live delegate; called whenever no live session is cached.
    redial: Redial,
    /// The mailbox this provider is bound to; used to report the sync scopes without
    /// touching the (possibly dead) delegate, exactly as [`crate::graph`] does for Graph.
    mailbox: MailboxId,
    /// Captured once from the initial connect, so the wrapper can still report data-domain
    /// support while no live session is cached.
    capabilities: Capabilities,
    /// The live delegate, or `None` after a retryable failure invalidated it (the next call
    /// re-dials). Held behind a std mutex read only to clone the `Arc`; never across an
    /// `.await`; like `RefreshingGraphProvider`'s `cached`.
    cached: Mutex<Option<Arc<dyn Provider>>>,
}

impl core::fmt::Debug for ReconnectingImapProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReconnectingImapProvider")
            .field("mailbox", &self.mailbox)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl ReconnectingImapProvider {
    /// Adopts an already-connected `initial` provider bound to `mailbox`, capturing its
    /// capabilities and the `redial` closure that rebuilds a fresh session after a drop. No
    /// second connect happens: the initial session is used until it fails.
    pub(crate) fn adopt(initial: Arc<dyn Provider>, mailbox: MailboxId, redial: Redial) -> Self {
        let capabilities = initial.connection_info().capabilities;
        Self {
            redial,
            mailbox,
            capabilities,
            cached: Mutex::new(Some(initial)),
        }
    }

    /// The live delegate, re-dialing a fresh session (and caching it) when none is held.
    async fn delegate(&self) -> ProviderResult<Arc<dyn Provider>> {
        {
            let cached = self.cached.lock().expect("imap delegate mutex poisoned");
            if let Some(provider) = cached.as_ref() {
                return Ok(Arc::clone(provider));
            }
        }
        let provider = (self.redial)().await?;
        *self.cached.lock().expect("imap delegate mutex poisoned") = Some(Arc::clone(&provider));
        Ok(provider)
    }

    /// Drops the cached delegate so the next [`delegate`](Self::delegate) re-dials; called
    /// after a retryable transport failure, whose socket is presumed dead.
    fn invalidate(&self) {
        *self.cached.lock().expect("imap delegate mutex poisoned") = None;
    }

    /// Runs `op` on the live delegate; on a [`FailureClass::Retryable`] failure (a dead
    /// socket), drops the session, re-dials, and retries `op` **once** on the fresh session.
    /// Only idempotent reads/edits use this: a dead socket means the command never reached
    /// the server, so a single retry is safe. `op` takes an owned `Arc`, so the retry holds
    /// nothing borrowed across the `.await`.
    async fn with_reconnect<T, F, Fut>(&self, op: F) -> ProviderResult<T>
    where
        F: Fn(Arc<dyn Provider>) -> Fut + Send,
        Fut: Future<Output = ProviderResult<T>> + Send,
    {
        let provider = self.delegate().await?;
        match op(Arc::clone(&provider)).await {
            Err(err) if err.class() == FailureClass::Retryable => {
                self.invalidate();
                let provider = self.delegate().await?;
                op(provider).await
            }
            result => result,
        }
    }
}

#[async_trait]
impl Provider for ReconnectingImapProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.cached
            .lock()
            .expect("imap delegate mutex poisoned")
            .as_ref()
            .map_or_else(
                || ConnectionInfo::new(self.capabilities),
                |provider| provider.connection_info(),
            )
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::ImapMailboxList {
            account: account.clone(),
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::ImapMailbox {
            account: account.clone(),
            mailbox: self.mailbox.clone(),
        }
    }

    async fn sync_mailboxes(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        let account = account.clone();
        let cursor = cursor.cloned();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let cursor = cursor.clone();
            async move { provider.sync_mailboxes(&account, cursor.as_ref()).await }
        })
        .await
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
            let mut retried = false;
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
                    Err(err) if !retried && err.class() == FailureClass::Retryable => {
                        self.invalidate();
                        retried = true;
                    }
                    Err(err) => Err(err)?,
                }
            }
        })
    }

    /// A mail write **is** safely auto-retried on a dropped socket: every [`MailEdit`] compiles to
    /// UID-addressed commands (`UID STORE`, `UID MOVE` per RFC 6851, `UID EXPUNGE`), and a
    /// non-existent UID is ignored without error (RFC 3501): so re-issuing an edit the server had
    /// already applied before the drop is a harmless no-op, not a double-apply. (It is also
    /// outbox-mediated, so even a hard failure is durably retried.) Contrast `submit_email`, which
    /// is genuinely non-idempotent and is excluded.
    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        let account = account.clone();
        let edit = edit.clone();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let edit = edit.clone();
            async move { provider.edit_mail(&account, &edit).await }
        })
        .await
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        let account = account.clone();
        let message = message.clone();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let message = message.clone();
            async move { provider.fetch_message_source(&account, &message).await }
        })
        .await
    }

    /// Submitting is **not** auto-retried: a send is not idempotent (a post-`DATA` drop may
    /// have delivered), so blind-retrying could double-send. A retryable failure only
    /// invalidates the session so the *next* explicit attempt re-dials; the error is
    /// returned for the caller (the durable outbox / the user) to decide.
    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        let provider = self.delegate().await?;
        match provider.submit_email(account, draft).await {
            Err(err) if err.class() == FailureClass::Retryable => {
                self.invalidate();
                Err(err)
            }
            result => result,
        }
    }

    /// IMAP reports a message by storing the `$Junk`/`$NotJunk` keyword, forwarded through the same
    /// redial helper as [`edit_mail`](Provider::edit_mail) so a dropped socket is redialled once
    /// rather than surfacing as a failed report.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        let account = account.clone();
        let report = report.clone();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let report = report.clone();
            async move { provider.report_message(&account, &report).await }
        })
        .await
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
