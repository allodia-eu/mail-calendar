//! The [`Provider`] trait implementation for [`ReconnectingGraphProvider`]: the stale-socket
//! reconnect loop wrapped around each delegated Graph call. Split from the module root (`super`),
//! which holds the struct, its delegate, and the connect entry points, to keep both files under
//! the 500-line cap.

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
    ScopeSync, SenderIdentity, SharedAccount, SubmissionReceipt,
};

use super::ReconnectingGraphProvider;

#[async_trait]
impl Provider for ReconnectingGraphProvider {
    fn connection_info(&self) -> ConnectionInfo {
        // Capped capabilities, the delegate's transport facts; see `delegate_info`.
        crate::delegate_info::with_delegate_transport(
            ConnectionInfo::new(self.capabilities),
            Some(self.delegate.last().connection_info()),
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
        self.delegate
            .retrying(|provider| async move { provider.sync_mailboxes(account, cursor).await })
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
        crate::stream_redial::redialling_stream(
            || std::future::ready(self.delegate.get()),
            |generation| self.delegate.drop_client(generation),
            account,
            cursor,
            window,
            fetch_batch,
            chunk_size,
        )
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        self.delegate
            .retrying(
                |provider| async move { provider.fetch_message_source(account, message).await },
            )
            .await
    }

    /// Applies a [`MailEdit`] (mark-read/flag, move, permanent delete) by delegating to the
    /// underlying [`GraphProvider`](provider_graph::GraphProvider), with the same stale-socket
    /// re-dial as the read calls. The `edit` is
    /// keyed by the message's immutable Graph id, so this folder-bound provider can edit a
    /// message in any folder. Safe to wrap like the idempotent reads; see
    /// [`should_reconnect`](crate::shared_client::should_reconnect) for why the re-dial can't
    /// double-apply the mutation.
    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        self.delegate
            .retrying(|provider| async move { provider.edit_mail(account, edit).await })
            .await
    }

    /// Submits a [`Draft`] via `POST /me/sendMail` by delegating to the underlying
    /// [`GraphProvider`](provider_graph::GraphProvider) (Graph files the Sent copy itself);
    /// **unlike** the reads and [`edit_mail`](Self::edit_mail), it **never re-issues the send on
    /// a transport failure.**
    ///
    /// A send is the one non-idempotent Graph call: re-`POST`ing `sendMail` delivers the
    /// message twice. Whether a failed attempt may have reached Graph is the outbox's to decide,
    /// from the hand-over record the adapter writes before the end of the request goes out
    /// (`hand_over`); it retries one that cannot have left and asks about one that may have. So
    /// on a transport error this only **drops** the (possibly dead-socketed) client, so
    /// the next attempt dials a fresh connection, then propagates the error instead of resending
    /// itself.
    ///
    /// A throttled send *is* re-issued, one layer down: Graph rejects it *before* sending, so
    /// the message never left and a replay cannot double-deliver.
    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
        hand_over: &HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        self.delegate
            .once(|provider| async move { provider.submit_email(account, draft, hand_over).await })
            .await
    }

    /// Saving a draft is **not** re-issued on a dropped connection, for the reason a send is not:
    /// it is a `POST` that creates a message, and one that landed before the socket died would be
    /// stored a second time by a replay. The outbox op behind it retries on a fresh connection
    /// instead.
    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
        metadata: &MessageMetadata,
    ) -> ProviderResult<DraftReceipt> {
        self.delegate
            .once(|provider| async move {
                provider
                    .put_draft(account, draft, replacing, metadata)
                    .await
            })
            .await
    }

    /// Removing one is retried on a dropped connection: the trait requires an absent draft to
    /// settle as done, so a repeat of a removal that already landed is a no-op.
    async fn delete_draft(&self, account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.delegate
            .retrying(|provider| async move { provider.delete_draft(account, draft).await })
            .await
    }

    async fn sender_identities(&self, account: &AccountId) -> ProviderResult<Vec<SenderIdentity>> {
        self.delegate
            .retrying(|provider| async move { provider.sender_identities(account).await })
            .await
    }

    /// Forwarded, because this wrapper advertises `ByAddress`: the only discovery Graph offers.
    /// A read, so it is made again after a dead connection.
    async fn resolve_shared_account(&self, address: &str) -> ProviderResult<SharedAccount> {
        self.delegate
            .retrying(|provider| async move { provider.resolve_shared_account(address).await })
            .await
    }

    /// Graph reports a message through `POST /messages/{id}/reportMessage`, forwarded on the same
    /// reconnect loop as [`edit_mail`](Provider::edit_mail), and counted against the account's
    /// ceiling by the engine like every other request. A report is idempotent, so a retry after a
    /// stale socket is safe.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        self.delegate
            .retrying(|provider| async move { provider.report_message(account, report).await })
            .await
    }
}

impl engine_api::CalendarSync for ReconnectingGraphProvider {}
impl CalendarWrites for ReconnectingGraphProvider {}

/// A folder change is made **once**: a dropped connection only drops the client, as for a send.
/// Re-issuing it is not safe on every transport (a rename replayed against the path it already
/// left meets nothing), and the outbox retries it anyway, after re-reading the folder list.
#[async_trait]
impl MailboxWrites for ReconnectingGraphProvider {
    async fn edit_mailbox(
        &self,
        account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        self.delegate
            .once(|provider| async move { provider.edit_mailbox(account, edit).await })
            .await
    }
}

#[cfg(test)]
#[path = "mail_provider_tests.rs"]
mod tests;
