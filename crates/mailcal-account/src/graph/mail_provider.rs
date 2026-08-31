//! The [`Provider`] trait implementation for [`RefreshingGraphProvider`]; the
//! token-refreshing, 429-backoff, and stale-socket reconnect loops wrapped around each
//! delegated Graph call. Split from the module root (`super`), which holds the struct, its
//! delegate cache, and the connect entry points, to keep both files under the 500-line cap.

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

use super::{RefreshingGraphProvider, should_reconnect};

#[async_trait]
impl Provider for RefreshingGraphProvider {
    fn connection_info(&self) -> ConnectionInfo {
        // Capped capabilities, the delegate's transport facts; see `delegate_info`.
        crate::delegate_info::with_delegate_transport(
            ConnectionInfo::new(self.capabilities),
            self.cached
                .lock()
                .expect("graph delegate mutex poisoned")
                .as_ref()
                .map(|(_, provider)| provider.connection_info()),
        )
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GraphFolderList {
            account: account.clone(),
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GraphFolder {
            account: account.clone(),
            folder: self.folder.clone(),
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
            let permit = self.tokens.acquire().await;
            match provider.sync_mailboxes(account, cursor).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    drop(permit);
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
                let permit = self.tokens.acquire().await;
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
                        drop(permit);
                        for chunk in chunks {
                            yield chunk;
                        }
                        break;
                    }
                    Err(err) if !reconnected && should_reconnect(&err) => {
                        drop(permit);
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
            let permit = self.tokens.acquire().await;
            match provider.fetch_message_source(account, message).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    drop(permit);
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Applies a [`MailEdit`] (mark-read/flag, move, permanent delete) by delegating to the
    /// underlying [`GraphProvider`](provider_graph::GraphProvider), with the same
    /// token-refresh, 429 backoff, and stale-socket re-dial as the read calls. The `edit` is
    /// keyed by the message's immutable Graph id, so this folder-bound provider can edit a
    /// message in any folder. Safe to wrap like the idempotent reads; see
    /// [`should_reconnect`] for why the re-dial can't double-apply the mutation.
    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            let permit = self.tokens.acquire().await;
            match provider.edit_mail(account, edit).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    drop(permit);
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Submits a [`Draft`] via `POST /me/sendMail` by delegating to the underlying
    /// [`GraphProvider`](provider_graph::GraphProvider) (Graph files the Sent copy itself),
    /// refreshing the access token first and backing off on a 429; but, **unlike** the reads
    /// and [`edit_mail`](Self::edit_mail), it **never re-issues the send on a transport
    /// failure.**
    ///
    /// A send is the one non-idempotent Graph call: re-`POST`ing `sendMail` delivers the
    /// message twice. A retryable transport error covers both a request that died at the
    /// socket *before* Graph accepted it (a resend would be safe) and one lost *after* Graph
    /// queued it (a resend double-sends): the two are indistinguishable here. The engine's
    /// outbox is built on exactly this: it calls `submit_email` once and never blind-retries,
    /// parking an ambiguous loss for the user to confirm rather than risk a duplicate
    /// (`engine-sync`'s `submit_mail`). So this wrapper honours that contract; on a transport
    /// error it only **drops** the (possibly dead-socketed) cached delegate, so the user's
    /// *deliberate* retry dials a fresh connection, then propagates the error instead of
    /// resending itself.
    ///
    /// A throttled send *is* re-issued, one layer down: Graph rejects it *before* sending, so
    /// the message never left and a replay cannot double-deliver.
    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        let provider = self.delegate().await?;
        let _permit = self.tokens.acquire().await;
        provider
            .submit_email(account, draft)
            .await
            .inspect_err(|err| {
                // A send is not idempotent and the outbox never blind-retries it, so this never
                // re-issues. Drop a possibly dead cached delegate (a keep-alive socket the OS
                // killed during sleep) so the user's deliberate retry rebuilds a fresh client,
                // then surface the error for the outbox to record.
                if should_reconnect(err) {
                    self.invalidate_delegate();
                }
            })
    }

    /// Graph reports a message through `POST /messages/{id}/reportMessage`, forwarded on the same
    /// token-refresh + reconnect loop as [`edit_mail`](Provider::edit_mail) and under the same
    /// concurrency permit. A report is idempotent, so a retry after a stale socket is safe.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        let mut provider = self.delegate().await?;
        let mut reconnected = false;
        loop {
            let permit = self.tokens.acquire().await;
            match provider.report_message(account, report).await {
                Ok(value) => return Ok(value),
                Err(err) if !reconnected && should_reconnect(&err) => {
                    drop(permit);
                    self.invalidate_delegate();
                    provider = self.delegate().await?;
                    reconnected = true;
                }
                Err(err) => return Err(err),
            }
        }
    }
}
