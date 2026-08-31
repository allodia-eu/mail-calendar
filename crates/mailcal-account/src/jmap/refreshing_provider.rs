//! The [`Provider`] implementation for [`RefreshingJmapProvider`]: every call mints/reuses a
//! live access token, then forwards to the delegate.
//!
//! Deliberately thin. Unlike the Graph and Google wrappers there is no rate-limit backoff and
//! no stale-socket reconnect loop here; those exist because Microsoft and Google throttle
//! aggressively and keep long-lived pools; the JMAP path has never needed them, and inventing
//! retry policy for servers we have not measured would be speculation. What this *does* add
//! is the one thing the JMAP path genuinely needs and cannot get anywhere else: a token that
//! is still valid on the thousandth call as well as the first.
//!
//! Split from the sibling `refreshing` module (which owns the struct and its delegate cache)
//! to keep both files under the 500-line cap.

use async_trait::async_trait;
use engine_core::{
    calendar::{Calendar, Event},
    ids::AccountId,
    mail::{Mailbox, Message},
    raw::RawMime,
    sync::{SyncScope, SyncState, SyncWindow},
};
use engine_provider::{
    ConnectionInfo, Draft, EmailStream, EventDeletion, EventDraft, EventEdit, EventWrite,
    EventWriteReceipt, MailEdit, MailEditReceipt, MessageReport, Provider, ProviderResult,
    ReportReceipt, ScopeSync, SubmissionReceipt,
};
use futures::StreamExt;

use super::refreshing::RefreshingJmapProvider;

#[async_trait]
impl Provider for RefreshingJmapProvider {
    fn connection_info(&self) -> ConnectionInfo {
        // The capabilities the initial session actually reported; never the wrapper's guess
        //; carrying the live delegate's transport facts (see `delegate_info`).
        crate::delegate_info::with_delegate_transport(
            ConnectionInfo::new(self.capabilities()),
            self.delegate_connection_info(),
        )
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        // The engine's default JMAP scopes are already the account-wide ones this provider
        // serves, so scope selection is delegated by *not* overriding it; see
        // `Provider::mailbox_scope`. Forwarding explicitly keeps the wrapper honest if the
        // delegate ever narrows them.
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::Mailbox,
        }
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::Email,
        }
    }

    fn default_sync_window(&self) -> SyncWindow {
        SyncWindow::full()
    }

    async fn sync_mailboxes(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        self.delegate().await?.sync_mailboxes(account, cursor).await
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
            // The delegate is resolved inside the stream (the token may have refreshed since
            // the stream was constructed), then its chunks are forwarded one for one.
            let provider = self.delegate().await?;
            let mut stream = provider.stream_email(
                &account,
                cursor.as_ref(),
                window,
                fetch_batch,
                chunk_size,
            );
            while let Some(chunk) = stream.next().await {
                yield chunk?;
            }
        })
    }

    async fn sync_email(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Message>> {
        self.delegate().await?.sync_email(account, cursor).await
    }

    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
    ) -> ProviderResult<SubmissionReceipt> {
        self.delegate().await?.submit_email(account, draft).await
    }

    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        self.delegate().await?.edit_mail(account, edit).await
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        self.delegate()
            .await?
            .fetch_message_source(account, message)
            .await
    }

    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::Calendar,
        }
    }

    fn event_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::CalendarEvent,
        }
    }

    async fn sync_calendars(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Calendar>> {
        self.delegate().await?.sync_calendars(account, cursor).await
    }

    async fn sync_events(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        self.delegate().await?.sync_events(account, cursor).await
    }

    async fn create_event(
        &self,
        account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        self.delegate().await?.create_event(account, draft).await
    }

    async fn patch_event(
        &self,
        account: &AccountId,
        base: &Event,
        edit: &EventEdit,
    ) -> ProviderResult<EventWriteReceipt> {
        self.delegate()
            .await?
            .patch_event(account, base, edit)
            .await
    }

    async fn put_event(
        &self,
        account: &AccountId,
        write: &EventWrite,
    ) -> ProviderResult<EventWriteReceipt> {
        self.delegate().await?.put_event(account, write).await
    }

    async fn delete_event(
        &self,
        account: &AccountId,
        base: Option<&Event>,
        deletion: &EventDeletion,
    ) -> ProviderResult<()> {
        self.delegate()
            .await?
            .delete_event(account, base, deletion)
            .await
    }

    /// JMAP reports a message by setting the RFC 8621 `$junk`/`$notjunk` keyword, forwarded to the
    /// session-backed delegate exactly as [`edit_mail`](Provider::edit_mail) is.
    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        self.delegate().await?.report_message(account, report).await
    }
}
