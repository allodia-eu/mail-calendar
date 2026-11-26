//! The calendar and contacts halves of [`ReconnectingJmapProvider`]: a read made once more on a
//! provider connected again after a dead connection, a write made once.

use async_trait::async_trait;
use engine_core::{
    calendar::Event,
    contact::{AddressBook, ContactCard, ContactDraft, ContactPatch, ContactResource},
    ids::{AccountId, CalendarId, ContactId},
    sync::SyncState,
    time::Horizon,
};
use engine_provider::{
    CalendarListing, CalendarSync, CalendarUserAddresses, CalendarWrites, ContactDestination,
    ContactPhoto, ContactSourceSync, ContactWriteReceipt, ContactsProvider, EventDeletion,
    EventDraft, EventEdit, EventRsvp, EventWriteReceipt, ProviderResult, ScopeSync,
};

use super::reconnecting::ReconnectingJmapProvider;

#[async_trait]
impl CalendarSync for ReconnectingJmapProvider {
    async fn sync_calendars(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<CalendarListing> {
        self.provider
            .retrying(|provider| async move { provider.sync_calendars(account, cursor).await })
            .await
    }

    async fn sync_events(
        &self,
        account: &AccountId,
        calendar: &CalendarId,
        cursor: Option<&SyncState>,
        window: Horizon,
    ) -> ProviderResult<ScopeSync<Event>> {
        self.provider
            .retrying(|provider| async move {
                provider
                    .sync_events(account, calendar, cursor, window)
                    .await
            })
            .await
    }
}

#[async_trait]
impl CalendarWrites for ReconnectingJmapProvider {
    async fn create_event(
        &self,
        account: &AccountId,
        draft: &EventDraft,
    ) -> ProviderResult<EventWriteReceipt> {
        self.provider
            .once(|provider| async move { provider.create_event(account, draft).await })
            .await
    }

    async fn patch_event(
        &self,
        account: &AccountId,
        base: &Event,
        edit: &EventEdit,
    ) -> ProviderResult<EventWriteReceipt> {
        self.provider
            .once(|provider| async move { provider.patch_event(account, base, edit).await })
            .await
    }

    async fn rsvp_event(
        &self,
        account: &AccountId,
        base: &Event,
        rsvp: &EventRsvp,
    ) -> ProviderResult<EventWriteReceipt> {
        self.provider
            .once(|provider| async move { provider.rsvp_event(account, base, rsvp).await })
            .await
    }

    async fn delete_event(
        &self,
        account: &AccountId,
        base: Option<&Event>,
        deletion: &EventDeletion,
    ) -> ProviderResult<()> {
        self.provider
            .once(|provider| async move { provider.delete_event(account, base, deletion).await })
            .await
    }

    async fn calendar_user_addresses(
        &self,
        account: &AccountId,
    ) -> ProviderResult<CalendarUserAddresses> {
        self.provider
            .retrying(|provider| async move { provider.calendar_user_addresses(account).await })
            .await
    }
}

#[async_trait]
impl ContactsProvider for ReconnectingJmapProvider {
    /// The provider's own answer, so the field set offered is the one the write is validated
    /// against.
    fn contact_destination(&self) -> Option<ContactDestination> {
        self.provider.last().contact_destination()
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.provider
            .retrying(|provider| async move { provider.sync_address_books(account, cursor).await })
            .await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.provider
            .retrying(|provider| async move { provider.sync_contacts(account, cursor).await })
            .await
    }

    async fn fetch_contact(
        &self,
        account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.provider
            .retrying(|provider| async move { provider.fetch_contact(account, contact).await })
            .await
    }

    async fn create_contact(
        &self,
        account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.provider
            .once(|provider| async move { provider.create_contact(account, draft).await })
            .await
    }

    async fn patch_contact(
        &self,
        account: &AccountId,
        base: &ContactCard,
        patch: &ContactPatch,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.provider
            .once(|provider| async move { provider.patch_contact(account, base, patch).await })
            .await
    }

    async fn delete_contact(&self, account: &AccountId, base: &ContactCard) -> ProviderResult<()> {
        self.provider
            .once(|provider| async move { provider.delete_contact(account, base).await })
            .await
    }

    async fn fetch_contact_photo(
        &self,
        account: &AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        self.provider
            .retrying(
                |provider| async move { provider.fetch_contact_photo(account, card, media).await },
            )
            .await
    }
}
