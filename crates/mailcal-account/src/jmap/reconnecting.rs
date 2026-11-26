//! The provider a JMAP account syncs through: the engine's [`JmapProvider`], connected again
//! after a request found its connection dead (`crate::redialled`).
//!
//! A JMAP provider holds one HTTP client for its life. A keep-alive socket the OS closed while the
//! machine slept fails the next request as `Retryable`, and the pool that held it may hold more
//! like it, so the provider is connected again, which fetches the session anew on a new client.
//! A read is then made once more; a write is made once, since it may have been applied before the
//! connection died, and only the next request goes out on the new provider. Every connect binds
//! the same contact book and shared mailbox, so a provider connected again writes where the
//! first did. The calendar and contacts halves are in `reconnecting_pim`, and the out-of-office
//! reply and the mail rules in `reconnecting_settings`. The importers (`mail_importer`,
//! `rules_importer`) are deliberately not forwarded, because they exist only behind the engine's
//! `import` feature and the app never enables it. Enabling that feature must forward them here
//! too, or this wrapper answers `None` where the engine's provider offers them.

use async_trait::async_trait;
use engine_api::{DraftReceipt, ProviderKey};
use engine_core::{
    ids::AccountId,
    mail::{Mailbox, Message, MessageMetadata},
    raw::RawMime,
    sync::{MailboxWindows, SyncScope, SyncState, Widening},
};
use engine_provider::{
    ConnectionInfo, Draft, EmailStream, HandOver, MailEdit, MailEditReceipt, MailRules,
    MailboxEdit, MailboxEditReceipt, MailboxWrites, MessageReport, Provider, ProviderResult,
    ReportReceipt, ScopeSync, SenderIdentity, SenderIdentityId, SharedAccount, SourceStream,
    SubmissionReceipt, Vacation,
};
use futures::StreamExt;
use provider_jmap::JmapProvider;

use crate::{
    redialled::{Redial, Redialled},
    shared_client::should_reconnect,
};

/// A JMAP account's provider, connected again after a dead connection. Internal; the account
/// layer hands callers `Box<dyn Provider>` and `Box<dyn ContactsProvider>`.
#[derive(Debug)]
pub(crate) struct ReconnectingJmapProvider {
    pub(super) provider: Redialled<JmapProvider>,
}

impl ReconnectingJmapProvider {
    /// Starts with `connected`; `redial` connects the next one as the first was.
    pub(super) fn adopt(connected: JmapProvider, redial: Redial<JmapProvider>) -> Self {
        Self {
            provider: Redialled::adopt(connected, redial),
        }
    }
}

#[async_trait]
impl Provider for ReconnectingJmapProvider {
    fn connection_info(&self) -> ConnectionInfo {
        self.provider.last().connection_info()
    }

    fn mailbox_scope(&self, account: &AccountId) -> SyncScope {
        self.provider.last().mailbox_scope(account)
    }

    fn email_scope(&self, account: &AccountId) -> SyncScope {
        self.provider.last().email_scope(account)
    }

    async fn sync_mailboxes(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ScopeSync<Mailbox>> {
        self.provider
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
            || self.provider.get(),
            |generation| self.provider.drop_dead(generation),
            account,
            cursor,
            window,
            fetch_batch,
            chunk_size,
        )
    }

    /// Walks the bands a widening adds on the provider in use, once: a pass that met a dead
    /// connection drops the provider and ends, and the next pass widens from the state it kept.
    fn widen_email<'a>(
        &'a self,
        account: &'a AccountId,
        cursor: &'a SyncState,
        widening: &'a Widening,
        fetch_batch: usize,
        chunk_size: usize,
    ) -> EmailStream<'a> {
        Box::pin(async_stream::try_stream! {
            let (generation, provider) = self.provider.get().await?;
            let mut bands =
                provider.widen_email(account, cursor, widening, fetch_batch, chunk_size);
            while let Some(chunk) = bands.next().await {
                match chunk {
                    Ok(chunk) => yield chunk,
                    Err(err) => {
                        if should_reconnect(&err) {
                            self.provider.drop_dead(generation);
                        }
                        Err(err)?;
                    }
                }
            }
        })
    }

    async fn fetch_message_source(
        &self,
        account: &AccountId,
        message: &Message,
    ) -> ProviderResult<RawMime> {
        self.provider
            .retrying(
                |provider| async move { provider.fetch_message_source(account, message).await },
            )
            .await
    }

    /// Forwarded to the provider's batch read, once: a message the batch could not deliver is
    /// read again by the caller. A dead connection drops the provider for the next request, and a
    /// provider that cannot be connected answers each message with that failure, every fact a
    /// caller reads kept ([`ProviderError::reworded`](engine_provider::ProviderError::reworded)).
    fn fetch_message_sources<'a>(
        &'a self,
        account: &'a AccountId,
        messages: &'a [Message],
    ) -> SourceStream<'a> {
        Box::pin(async_stream::stream! {
            let (generation, provider) = match self.provider.get().await {
                Ok(current) => current,
                Err(err) => {
                    for index in 0..messages.len() {
                        yield (index, Err(err.reworded(err.detail())));
                    }
                    return;
                }
            };
            let mut lost = false;
            {
                let mut sources = provider.fetch_message_sources(account, messages);
                while let Some((index, result)) = sources.next().await {
                    lost |= matches!(&result, Err(err) if should_reconnect(err));
                    yield (index, result);
                }
            }
            if lost {
                self.provider.drop_dead(generation);
            }
        })
    }

    async fn submit_email(
        &self,
        account: &AccountId,
        draft: &Draft,
        hand_over: &HandOver<'_>,
    ) -> ProviderResult<SubmissionReceipt> {
        self.provider
            .once(|provider| async move { provider.submit_email(account, draft, hand_over).await })
            .await
    }

    async fn put_draft(
        &self,
        account: &AccountId,
        draft: &Draft,
        replacing: Option<&ProviderKey>,
        metadata: &MessageMetadata,
    ) -> ProviderResult<DraftReceipt> {
        self.provider
            .once(|provider| async move {
                provider
                    .put_draft(account, draft, replacing, metadata)
                    .await
            })
            .await
    }

    async fn delete_draft(&self, account: &AccountId, draft: &ProviderKey) -> ProviderResult<()> {
        self.provider
            .once(|provider| async move { provider.delete_draft(account, draft).await })
            .await
    }

    async fn edit_mail(
        &self,
        account: &AccountId,
        edit: &MailEdit,
    ) -> ProviderResult<MailEditReceipt> {
        self.provider
            .once(|provider| async move { provider.edit_mail(account, edit).await })
            .await
    }

    async fn sender_identities(&self, account: &AccountId) -> ProviderResult<Vec<SenderIdentity>> {
        self.provider
            .retrying(|provider| async move { provider.sender_identities(account).await })
            .await
    }

    async fn set_sender_name(
        &self,
        account: &AccountId,
        identity: &SenderIdentityId,
        name: &str,
    ) -> ProviderResult<()> {
        self.provider
            .once(|provider| async move { provider.set_sender_name(account, identity, name).await })
            .await
    }

    async fn list_shared_accounts(&self) -> ProviderResult<Vec<SharedAccount>> {
        self.provider
            .retrying(|provider| async move { provider.list_shared_accounts().await })
            .await
    }

    async fn report_message(
        &self,
        account: &AccountId,
        report: &MessageReport,
    ) -> ProviderResult<ReportReceipt> {
        self.provider
            .once(|provider| async move { provider.report_message(account, report).await })
            .await
    }

    /// This wrapper, where the provider connected last offers a reply (`reconnecting_settings`).
    fn vacation(&self) -> Option<&dyn Vacation> {
        self.provider.last().vacation().is_some().then_some(self)
    }

    /// This wrapper, where the provider connected last offers rules (`reconnecting_settings`).
    fn mail_rules(&self) -> Option<&dyn MailRules> {
        self.provider.last().mail_rules().is_some().then_some(self)
    }
}

#[async_trait]
impl MailboxWrites for ReconnectingJmapProvider {
    async fn edit_mailbox(
        &self,
        account: &AccountId,
        edit: &MailboxEdit,
    ) -> ProviderResult<MailboxEditReceipt> {
        self.provider
            .once(|provider| async move { provider.edit_mailbox(account, edit).await })
            .await
    }
}

#[cfg(test)]
#[path = "reconnecting_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "reconnecting_stream_tests.rs"]
mod stream_tests;
