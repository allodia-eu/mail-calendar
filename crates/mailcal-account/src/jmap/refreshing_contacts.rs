//! The [`ContactsProvider`] implementation for [`RefreshingJmapProvider`]: the same
//! mint-a-live-token-then-forward shape as its [`Provider`] sibling, for the contacts half
//! of the surface.
//!
//! This exists for the same reason the sibling does: an OAuth JMAP account (Fastmail) hands
//! the app a wrapper, not a bare `JmapProvider`, and the wrapper is what the app syncs
//! through. Without this impl that account would report *no* contacts support: every
//! [`ContactsProvider`] method has a default body that returns an error, so the omission
//! would not fail to compile, it would quietly look like a server that has no address books.
//!
//! Split from `refreshing_provider.rs` by responsibility (and to keep both under the
//! 500-line cap).
//!
//! [`Provider`]: engine_provider::Provider

use async_trait::async_trait;
use engine_core::{
    contact::{AddressBook, ContactCard, ContactDraft, ContactPatch, ContactResource},
    ids::{AccountId, ContactId},
    sync::SyncState,
};
use engine_provider::{
    ContactDestination, ContactPhoto, ContactSourceSync, ContactWriteReceipt, ContactsProvider,
    ProviderResult,
};

use super::refreshing::RefreshingJmapProvider;

#[async_trait]
impl ContactsProvider for RefreshingJmapProvider {
    fn contact_destination(&self) -> Option<ContactDestination> {
        // Forwarded from the cached delegate rather than rebuilt here: which address book a
        // write lands in is learned from the session and known only to the delegate, so a
        // wrapper that synthesized a destination would be naming a collection it guessed.
        self.delegate_contact_destination()
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.delegate()
            .await?
            .sync_address_books(account, cursor)
            .await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.delegate().await?.sync_contacts(account, cursor).await
    }

    async fn fetch_contact(
        &self,
        account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.delegate().await?.fetch_contact(account, contact).await
    }

    async fn create_contact(
        &self,
        account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.delegate().await?.create_contact(account, draft).await
    }

    async fn patch_contact(
        &self,
        account: &AccountId,
        base: &ContactCard,
        patch: &ContactPatch,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.delegate()
            .await?
            .patch_contact(account, base, patch)
            .await
    }

    async fn delete_contact(&self, account: &AccountId, base: &ContactCard) -> ProviderResult<()> {
        self.delegate().await?.delete_contact(account, base).await
    }

    async fn fetch_contact_photo(
        &self,
        account: &AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        self.delegate()
            .await?
            .fetch_contact_photo(account, card, media)
            .await
    }
}
