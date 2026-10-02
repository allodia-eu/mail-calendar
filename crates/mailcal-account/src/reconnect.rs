//! An IMAP [`Provider`] wrapper that retries a call once after its socket died.
//!
//! The engine's `ImapProvider` borrows a connection from its account per call and replaces a
//! dead one on the *next* call, but the call that found it dead has already failed with a
//! [`FailureClass::Retryable`] transport error (`Broken pipe`, `peer closed connection without
//! sending TLS close_notify`). After the machine slept or the network dropped, that is the first
//! thing the user does: a Refresh, or opening a message, and it would fail once for no reason
//! they can see.
//!
//! [`ReconnectingImapProvider`] retries it the same way [`RefreshingGraphProvider`] does on the
//! Graph side ([`crate::graph`]): it caches a live delegate and, on a retryable transport
//! failure, drops it, takes a fresh one, and retries the (idempotent) call once. The fresh one
//! comes from an **injected closure** ([`Redial`]) so the path is unit-testable without a real
//! socket; the live closure (`imap::make_imap_redial`) drops the account's resting connections
//! and binds the folder again, so the retry runs on a connection dialled for it.
//!
//! [`RefreshingGraphProvider`]: crate::graph

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use engine_api::{CalendarWrites, ProviderKey};
use engine_core::{
    error::FailureClass,
    ids::{AccountId, MailboxId},
    mail::{Mailbox, Message},
    raw::RawMime,
    sync::{SyncScope, SyncState, SyncWindow},
};
use engine_provider::{
    ConnectionInfo, Draft, EmailStream, MailEdit, MailEditReceipt, MailboxEdit, MailboxEditReceipt,
    MailboxWrites, MessageReport, Provider, ProviderError, ProviderResult, ReportReceipt,
    ScopeSync, SourceStream, SubmissionReceipt,
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
    /// What the live delegate last reported, so the wrapper still reports it while no session
    /// is cached. Not the defaults: those say one fetch at a time and one message per request,
    /// and a body warm that reads them in the moment between a lost connection and the redial
    /// runs its whole pass that way. The width describes the account's pool, which outlives
    /// any one session, so the last value is the right one to keep.
    last_info: Mutex<ConnectionInfo>,
    /// The live delegate, or `None` after a retryable failure invalidated it (the next call
    /// re-dials). Held behind a std mutex read only to clone the `Arc`; never across an
    /// `.await`; like `RefreshingGraphProvider`'s `cached`.
    cached: Mutex<Option<Arc<dyn Provider>>>,
}

impl core::fmt::Debug for ReconnectingImapProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReconnectingImapProvider")
            .field("mailbox", &self.mailbox)
            .field("last_info", &self.last_info)
            .finish_non_exhaustive()
    }
}

impl ReconnectingImapProvider {
    /// Adopts an already-connected `initial` provider bound to `mailbox`, capturing what it
    /// reports and the `redial` closure that rebuilds a fresh session after a drop. No
    /// second connect happens: the initial session is used until it fails.
    pub(crate) fn adopt(initial: Arc<dyn Provider>, mailbox: MailboxId, redial: Redial) -> Self {
        let last_info = Mutex::new(initial.connection_info());
        Self {
            redial,
            mailbox,
            last_info,
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
        let live = self
            .cached
            .lock()
            .expect("imap delegate mutex poisoned")
            .as_ref()
            .map(|provider| provider.connection_info());
        let mut last = self.last_info.lock().expect("imap info mutex poisoned");
        if let Some(live) = live {
            *last = live;
        }
        *last
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

    /// Forwarded to the delegate, whose batch is what makes a warm fast: without this the
    /// trait's default would fetch the batch one message at a time. Not retried as a whole,
    /// because the engine already asks once more on a fresh connection for what a lost one had
    /// not delivered, and repeating the batch here would fetch again what already arrived. A
    /// lost connection still drops the session, so the next call redials, as a single fetch's
    /// would.
    fn fetch_message_sources<'a>(
        &'a self,
        account: &'a AccountId,
        messages: &'a [Message],
    ) -> SourceStream<'a> {
        Box::pin(async_stream::stream! {
            let provider = match self.delegate().await {
                Ok(provider) => provider,
                Err(err) => {
                    for index in 0..messages.len() {
                        yield (index, Err(ProviderError::new(err.class(), err.to_string())));
                    }
                    return;
                }
            };
            let mut lost = false;
            {
                let mut sources = provider.fetch_message_sources(account, messages);
                while let Some((index, result)) = sources.next().await {
                    lost |= matches!(&result, Err(err) if err.class() == FailureClass::Retryable);
                    yield (index, result);
                }
            }
            if lost {
                self.invalidate();
            }
        })
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

    /// Saving a draft is **not** auto-retried, for the reason a send is not: it is an `APPEND`,
    /// and one that landed before the socket died would be stored a second time by a replay.
    /// The outbox op behind it retries on a fresh session instead.
    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
    ) -> ProviderResult<ProviderKey> {
        let provider = self.delegate().await?;
        match provider.put_draft(account, draft, replacing).await {
            Err(err) if err.class() == FailureClass::Retryable => {
                self.invalidate();
                Err(err)
            }
            result => result,
        }
    }

    /// Removing one is retried: the trait requires an absent draft to settle as done, so a
    /// repeat of a removal that already landed is a no-op.
    async fn delete_draft(&self, account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        let account = account.clone();
        let draft = draft.clone();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let draft = draft.clone();
            async move { provider.delete_draft(&account, &draft).await }
        })
        .await
    }

    /// Filing the sender's copy of a delivered message is retried on a dropped socket, unlike the
    /// send itself: the trait requires the filing to look for the copy before placing one, so a
    /// repeat finds what the first attempt placed rather than storing it twice.
    async fn file_sent_copy(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<ProviderKey> {
        let account = account.clone();
        let draft = draft.clone();
        self.with_reconnect(move |provider| {
            let account = account.clone();
            let draft = draft.clone();
            async move { provider.file_sent_copy(&account, &draft).await }
        })
        .await
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

impl CalendarWrites for ReconnectingImapProvider {}

/// A folder change is **not** auto-retried on a dropped socket, unlike [`edit_mail`]: a
/// `RENAME` replayed after it landed meets a path that no longer exists and reads as a
/// conflict. A retryable failure only invalidates the session; the outbox retries the change
/// after re-reading the folder list, which is what makes a repeat safe.
///
/// [`edit_mail`]: Provider::edit_mail
#[async_trait]
impl MailboxWrites for ReconnectingImapProvider {
    async fn edit_mailbox(
        &self,
        account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        let provider = self.delegate().await?;
        match provider.edit_mailbox(account, edit).await {
            Err(err) if err.class() == FailureClass::Retryable => {
                self.invalidate();
                Err(err)
            }
            result => result,
        }
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
