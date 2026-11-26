//! The contact provider a Graph or Google account binds per source: the engine's adapter for that
//! source on the account's shared client, a read tried once more on a new client after it failed
//! on a dead connection, and a write made once.
//!
//! **The delegate is replaced, never emptied**: it also answers the trait's synchronous questions
//! (both sync scopes and the write destination), and those must not fall through to
//! [`ContactsProvider`]'s JMAP-shaped defaults, which would key every source's cursor to one
//! scope.

use async_trait::async_trait;
use engine_core::{
    contact::{AddressBook, ContactCard, ContactDraft, ContactPatch, ContactResource},
    ids::{AccountId, ContactId},
    sync::{SyncScope, SyncState},
};
use engine_provider::{
    CalendarWrites, ConnectionInfo, ContactDestination, ContactPhoto, ContactSourceSync,
    ContactWriteReceipt, ContactsProvider, Provider, ProviderResult,
};

use crate::shared_client::Delegate;

/// A [`ContactsProvider`] for one source, forwarding to the engine's adapter `P` on a client `C`.
/// Internal; the account layer hands callers `Box<dyn ContactsProvider>`.
pub(crate) struct ReconnectingContacts<C, P> {
    pub(crate) delegate: Delegate<C, P>,
}

impl<C, P: core::fmt::Debug> core::fmt::Debug for ReconnectingContacts<C, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReconnectingContacts")
            .field("delegate", &self.delegate)
            .finish()
    }
}

#[async_trait]
impl<C, P> Provider for ReconnectingContacts<C, P>
where
    C: Clone + Send + Sync + 'static,
    P: ContactsProvider + 'static,
{
    fn connection_info(&self) -> ConnectionInfo {
        self.delegate.last().connection_info()
    }
}

impl<C, P> engine_api::MailboxWrites for ReconnectingContacts<C, P>
where
    C: Clone + Send + Sync + 'static,
    P: ContactsProvider + 'static,
{
}

impl<C, P> engine_api::CalendarSync for ReconnectingContacts<C, P>
where
    C: Clone + Send + Sync + 'static,
    P: ContactsProvider + 'static,
{
}

impl<C, P> CalendarWrites for ReconnectingContacts<C, P>
where
    C: Clone + Send + Sync + 'static,
    P: ContactsProvider + 'static,
{
}

#[async_trait]
impl<C, P> ContactsProvider for ReconnectingContacts<C, P>
where
    C: Clone + Send + Sync + 'static,
    P: ContactsProvider + 'static,
{
    fn address_book_scope(&self, account: &AccountId) -> SyncScope {
        self.delegate.last().address_book_scope(account)
    }

    fn contact_scope(&self, account: &AccountId) -> SyncScope {
        self.delegate.last().contact_scope(account)
    }

    fn contact_destination(&self) -> Option<ContactDestination> {
        // The adapter's own answer, so the field set offered is the one the write is
        // validated against.
        self.delegate.last().contact_destination()
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.delegate
            .retrying(|provider| async move { provider.sync_address_books(account, cursor).await })
            .await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.delegate
            .retrying(|provider| async move { provider.sync_contacts(account, cursor).await })
            .await
    }

    async fn fetch_contact(
        &self,
        account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.delegate
            .retrying(|provider| async move { provider.fetch_contact(account, contact).await })
            .await
    }

    async fn create_contact(
        &self,
        account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        // A `POST`: a blind retry could create the card twice.
        self.delegate
            .once(|provider| async move { provider.create_contact(account, draft).await })
            .await
    }

    async fn patch_contact(
        &self,
        account: &AccountId,
        base: &ContactCard,
        patch: &ContactPatch,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.delegate
            .once(|provider| async move { provider.patch_contact(account, base, patch).await })
            .await
    }

    async fn delete_contact(&self, account: &AccountId, base: &ContactCard) -> ProviderResult<()> {
        self.delegate
            .once(|provider| async move { provider.delete_contact(account, base).await })
            .await
    }

    async fn fetch_contact_photo(
        &self,
        account: &AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        self.delegate
            .retrying(
                |provider| async move { provider.fetch_contact_photo(account, card, media).await },
            )
            .await
    }
}
