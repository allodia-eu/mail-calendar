//! The [`Provider`] trait implementation for `ReconnectingGmailProvider`: the stale-socket
//! reconnect loop wrapped around each delegated Gmail call, plus the two write paths
//! (`edit_mail`, `submit_email`). Split from the module root (`super`), which holds the struct,
//! its delegates, and the connect entry points, to keep both files under the 500-line cap;
//! mirroring [`crate::graph`]'s identical split.

use async_trait::async_trait;
use engine_api::{CalendarWrites, DraftReceipt, ProviderKey};
use engine_core::{
    ids::AccountId,
    mail::{Mailbox, Message, MessageMetadata},
    raw::RawMime,
    sync::{MailboxWindows, SyncScope, SyncState},
};
use engine_provider::{
    ConnectionInfo, Draft, EmailStream, HandOver, MailEdit, MailEditReceipt, MailboxEdit,
    MailboxEditReceipt, MailboxWrites, MessageReport, Provider, ProviderResult, ReportReceipt,
    ScopeSync, SenderIdentity, SenderIdentityId, SourceStream, SubmissionReceipt,
};
use futures::StreamExt;

use super::ReconnectingGmailProvider;

#[async_trait]
impl Provider for ReconnectingGmailProvider {
    fn connection_info(&self) -> ConnectionInfo {
        // Capped capabilities, the delegate's transport facts; see `delegate_info`.
        crate::delegate_info::with_delegate_transport(
            ConnectionInfo::new(self.capabilities),
            [&self.cached_reader, &self.cached]
                .into_iter()
                .find_map(|cache| {
                    cache
                        .lock()
                        .expect("gmail delegate mutex poisoned")
                        .as_ref()
                        .map(|built| built.provider.connection_info())
                }),
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
        self.retrying(false, |provider| async move {
            provider.sync_mailboxes(account, cursor).await
        })
        .await
    }

    fn stream_email<'a>(
        &'a self,
        account: &'a AccountId,
        cursor: Option<&'a SyncState>,
        window: MailboxWindows,
        fetch_batch: usize,
        chunk_size: usize,
    ) -> EmailStream<'a> {
        let reads = crate::stream_redial::redialling_stream(
            || self.reading_delegate(),
            |generation| self.drop_client(generation),
            account,
            cursor,
            window,
            fetch_batch,
            chunk_size,
        );
        // How each read ends is what tells a network that blocks IMAP from one that is down: the
        // API answers on the first, so nothing but these reads notices mail has stopped.
        Box::pin(reads.inspect(|read| {
            self.imap.note_read(read.as_ref().map(|_| ()));
        }))
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        self.retrying(true, |provider| async move {
            provider.fetch_message_source(account, message).await
        })
        .await
    }

    /// Forwards the batch read where the delegate carries several sources a request (fifty over
    /// IMAP); otherwise reads one at a time through
    /// [`fetch_message_source`](Self::fetch_message_source), which re-dials a stale socket once
    /// per message. A delegate that cannot be had answers each message with its failure, every
    /// fact a caller reads kept
    /// ([`ProviderError::reworded`](engine_provider::ProviderError::reworded)).
    fn fetch_message_sources<'a>(
        &'a self,
        account: &'a AccountId,
        messages: &'a [Message],
    ) -> SourceStream<'a> {
        Box::pin(async_stream::stream! {
            match self.reading_delegate().await {
                Ok((_, provider)) if provider.connection_info().sources_per_request > 1 => {
                    let mut sources = provider.fetch_message_sources(account, messages);
                    while let Some(item) = sources.next().await {
                        yield item;
                    }
                }
                Ok(_) => {
                    let mut sources = engine_provider::one_at_a_time(self, account, messages);
                    while let Some(item) = sources.next().await {
                        yield item;
                    }
                }
                Err(err) => {
                    for index in 0..messages.len() {
                        yield (index, Err(err.reworded(err.detail())));
                    }
                }
            }
        })
    }

    /// Applies a [`MailEdit`] (mark-read/flag, move/archive, permanent delete) by delegating
    /// to the underlying `GmailProvider`, with the same stale-socket re-dial as the read calls.
    /// Gmail's edits are keyed by the message's immutable Gmail id, and this provider is
    /// account-global, so it can edit a message under any label.
    ///
    /// Safe to wrap like the idempotent reads: a Gmail edit is a label **delta** to a fixed
    /// state (`modify`/`trash`/`delete`), so re-applying it lands on the same result; see
    /// [`should_reconnect`](crate::shared_client::should_reconnect).
    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        self.retrying(false, |provider| async move {
            provider.edit_mail(account, edit).await
        })
        .await
    }

    /// Submits a [`Draft`] via `messages.send` by delegating to the underlying
    /// `GmailProvider` (Gmail files the Sent copy itself); **unlike** the reads and
    /// [`edit_mail`](Self::edit_mail), it **never re-issues the send on a transport failure**.
    ///
    /// A send is the one non-idempotent Gmail call: re-`POST`ing `messages.send` delivers the
    /// message twice. Whether a failed attempt may have reached Gmail is the outbox's to decide,
    /// from the hand-over record the adapter writes before the end of the request goes out
    /// (`hand_over`); it retries one that cannot have left and asks about one that may have. So
    /// on a transport error this only **drops** the (possibly dead-socketed) client, so
    /// the next attempt dials a fresh connection, then propagates the error.
    ///
    /// A throttled send *is* re-issued, one layer down: Google rejects it *before* acting on
    /// it, so the message never left and a replay cannot double-deliver. Mirrors the identical
    /// rule in [`crate::graph`]'s wrapper.
    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
        hand_over: &HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        self.once(|provider| async move { provider.submit_email(account, draft, hand_over).await })
            .await
    }

    /// Saving a draft is **not** re-issued on a dropped connection, for the reason a send is not:
    /// a first save is a `drafts.create`, and one that landed before the socket died would be
    /// stored a second time by a replay. The outbox op behind it retries on a fresh connection
    /// instead.
    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
        metadata: &MessageMetadata,
    ) -> ProviderResult<DraftReceipt> {
        self.once(|provider| async move {
            provider
                .put_draft(account, draft, replacing, metadata)
                .await
        })
        .await
    }

    /// Removing one is retried on a dropped connection: the trait requires an absent draft to
    /// settle as done, so a repeat of a removal that already landed is a no-op.
    async fn delete_draft(&self, account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.retrying(false, |provider| async move {
            provider.delete_draft(account, draft).await
        })
        .await
    }

    async fn sender_identities(&self, account: &AccountId) -> ProviderResult<Vec<SenderIdentity>> {
        self.retrying(false, |provider| async move {
            provider.sender_identities(account).await
        })
        .await
    }

    async fn set_sender_name(
        &self,
        account: &AccountId,
        identity: &SenderIdentityId,
        name: &str,
    ) -> ProviderResult<()> {
        self.retrying(false, |provider| async move {
            provider.set_sender_name(account, identity, name).await
        })
        .await
    }

    /// Gmail reports a message by moving it under its `SPAM` label, which this forwards on the
    /// same reconnect loop [`edit_mail`](Provider::edit_mail) uses: a report is idempotent, so a
    /// retry after a stale socket cannot double-apply.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        self.retrying(false, |provider| async move {
            provider.report_message(account, report).await
        })
        .await
    }
}

impl engine_api::CalendarSync for ReconnectingGmailProvider {}
impl CalendarWrites for ReconnectingGmailProvider {}

/// A folder change is made **once**: a dropped connection only drops the client, as for a send.
/// Re-issuing it is not safe on every transport (a rename replayed against the path it already
/// left meets nothing), and the outbox retries it anyway, after re-reading the folder list.
#[async_trait]
impl MailboxWrites for ReconnectingGmailProvider {
    async fn edit_mailbox(
        &self,
        account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        self.once(|provider| async move { provider.edit_mailbox(account, edit).await })
            .await
    }
}

#[cfg(test)]
#[path = "mail_provider_tests.rs"]
mod tests;
