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
use engine_api::{CalendarWrites, ProviderKey};
use engine_core::{
    calendar::Event,
    ids::{AccountId, CalendarId},
    mail::{Mailbox, Message},
    raw::RawMime,
    sync::{SyncScope, SyncState, SyncWindow},
};
use engine_provider::{
    CalendarListing, CalendarUserAddresses, ConnectionInfo, Draft, EmailStream, EventDeletion,
    EventDraft, EventEdit, EventRsvp, EventWrite, EventWriteReceipt, HandOver, MailEdit,
    MailEditReceipt, MailboxEdit, MailboxEditReceipt, MailboxWrites, MessageReport, Provider,
    ProviderError, ProviderResult, ReportReceipt, ScopeSync, SenderIdentity, SenderIdentityId,
    SharedMailbox, SourceStream, SubmissionReceipt,
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
        hand_over: &HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        self.delegate()
            .await?
            .submit_email(account, draft, hand_over)
            .await
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

    /// Forwarded, because the delegate's batch is one `Blob/get` for the lot where the server
    /// offers it, and the trait's default would fetch the batch one message at a time through
    /// [`fetch_message_source`](Self::fetch_message_source) instead.
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
            let mut sources = provider.fetch_message_sources(account, messages);
            while let Some(item) = sources.next().await {
                yield item;
            }
        })
    }

    fn calendar_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::Calendar,
        }
    }

    fn event_scope(&self, account: &AccountId, _calendar: &CalendarId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::CalendarEvent,
        }
    }

    async fn sync_calendars(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<CalendarListing> {
        self.delegate().await?.sync_calendars(account, cursor).await
    }

    async fn sync_events(
        &self,
        account: &AccountId,
        calendar: &CalendarId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Event>> {
        self.delegate()
            .await?
            .sync_events(account, calendar, cursor)
            .await
    }

    /// The identities the session's submission account can send as, forwarded to the
    /// session-backed delegate.
    ///
    /// This wrapper reports the **session's own** capabilities rather than a capped set, so
    /// `sender_identities` is advertised whenever the server advertised submission. Advertising
    /// and forwarding move together: a verb left unforwarded here answers "unsupported" while
    /// the capability says otherwise, and a host reading the capability would draw an editor
    /// over it.
    async fn sender_identities(&self, account: &AccountId) -> ProviderResult<Vec<SenderIdentity>> {
        self.delegate().await?.sender_identities(account).await
    }

    async fn set_sender_name(
        &self,
        account: &AccountId,
        identity: &SenderIdentityId,
        name: &str,
    ) -> ProviderResult<()> {
        self.delegate()
            .await?
            .set_sender_name(account, identity, name)
            .await
    }

    /// Forwarded whenever the session advertised drafts, which is when this wrapper's capabilities
    /// say so: advertising and forwarding move together (see `sender_identities`).
    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
    ) -> ProviderResult<ProviderKey> {
        self.delegate()
            .await?
            .put_draft(account, draft, replacing)
            .await
    }

    async fn delete_draft(&self, account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.delegate().await?.delete_draft(account, draft).await
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

    /// Forwarded, because this wrapper reports the session's own capabilities, which say whether
    /// the session lists shared mailboxes (see `sender_identities`).
    async fn list_shared_mailboxes(&self) -> ProviderResult<Vec<SharedMailbox>> {
        self.delegate().await?.list_shared_mailboxes().await
    }
}

#[async_trait]
impl MailboxWrites for RefreshingJmapProvider {
    async fn edit_mailbox(
        &self,
        account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        self.delegate().await?.edit_mailbox(account, edit).await
    }
}

#[async_trait]
impl CalendarWrites for RefreshingJmapProvider {
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

    async fn rsvp_event(
        &self,
        account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        self.delegate().await?.rsvp_event(account, base, rsvp).await
    }

    async fn calendar_user_addresses(
        &self,
        account: &AccountId,
    ) -> ProviderResult<CalendarUserAddresses> {
        self.delegate()
            .await?
            .calendar_user_addresses(account)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use provider_jmap::{Credentials, JmapConfig, JmapProvider};

    use super::*;
    use crate::graph::test_support::{
        assert_calendar_answers_reach_the_delegate, dead_grant_token_endpoint, mock_token_endpoint,
        source_at,
    };

    /// The smallest session a calendar account connects with.
    const SESSION: &str = r#"{"capabilities":{"urn:ietf:params:jmap:core":{},"urn:ietf:params:jmap:calendars":{}},"primaryAccounts":{"urn:ietf:params:jmap:calendars":"c"},"accounts":{"c":{"accountCapabilities":{"urn:ietf:params:jmap:calendars":{}}}},"apiUrl":"https://mail.example.com/jmap/"}"#;

    /// An OAuth JMAP account answers invitations through this wrapper, which forwards each verb
    /// by hand; one it leaves out answers with the trait's unsupported default.
    #[tokio::test]
    async fn invitation_answers_are_forwarded_to_jmap() {
        let (server, _) = mock_token_endpoint(vec![SESSION.to_owned()]);
        let base_url = server.trim_end_matches("/token").to_owned();
        let initial = JmapProvider::connect(JmapConfig::new(
            base_url.clone(),
            Credentials::bearer("first-token".to_owned()),
            crate::user_agent(),
        ))
        .await
        .expect("the session connects");
        let capabilities = initial.connection_info().capabilities;
        let (endpoint, _hits) = dead_grant_token_endpoint();
        let provider = RefreshingJmapProvider::new(
            base_url,
            Arc::clone(&source_at(endpoint, None)),
            crate::tls::tls_with(&[]).unwrap(),
            "first-token".to_owned(),
            initial,
            capabilities,
            None,
        );
        assert_calendar_answers_reach_the_delegate(&provider).await;
    }
}
