//! The app both avatar suites drive: a real in-memory engine, one account, and a contacts
//! adapter serving a single card.
//!
//! Real rather than stubbed on purpose: the engine derives the people index and caches the
//! bytes, so a suite walks address → person → card → photo the way the app does. Stubbing any
//! of it would prove only that the stub still compiles.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use engine_api::{AccountId, CalendarWrites, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    contact::{
        AddressBook, ContactCard, ContactEmail, ContactName, ContactProperty, ContactResource,
        ContactSourceClass, PropertyId,
    },
    ids::{AddressBookId, ContactId},
    membership::Memberships,
    sync::{SyncScope, SyncState, SyncUpdate},
};
use engine_provider::{
    Capabilities, ConnectionInfo, ContactPhoto, ContactSourceSync, ContactsProvider, Provider,
    ProviderError, ProviderResult, ScopeSync,
};
use mailcal_viewmodel::{
    avatar,
    view::{FlatRow, MailboxListSnapshot, SnapshotRow},
};

use crate::{Account, App, AppObserver, Surface, Telemetry, TimeZoneInit};

/// A one-pixel PNG; real magic bytes, so the sniffing under test sees what it would in life.
pub(super) const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01";

pub(super) struct RecordingObserver {
    surfaces: Arc<Mutex<Vec<Surface>>>,
}

impl AppObserver for RecordingObserver {
    fn surface_changed(&self, surface: Surface) {
        self.surfaces.lock().unwrap().push(surface);
    }
}

/// A mail provider that serves no mail; these tests build their rows directly, because what
/// is under test is the photo pass, not the projection that feeds it.
pub(super) struct MailOnly;

impl Provider for MailOnly {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none())
    }
}

impl engine_api::MailboxWrites for MailOnly {}
impl CalendarWrites for MailOnly {}

/// A contacts adapter serving one card, which may carry a photo.
pub(super) struct FakeContacts {
    pub(super) card: ContactCard,
    pub(super) photo: Option<Vec<u8>>,
}

impl Provider for FakeContacts {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_contacts())
    }
}

impl engine_api::MailboxWrites for FakeContacts {}
impl CalendarWrites for FakeContacts {}

#[async_trait]
impl ContactsProvider for FakeContacts {
    async fn sync_address_books(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        let book = AddressBookId::try_from("book").unwrap();
        Ok(ContactSourceSync::Available {
            sync: ScopeSync::new(
                SyncUpdate::snapshot(
                    vec![AddressBook::new(
                        book.clone(),
                        "Contacts",
                        ContactSourceClass::Personal,
                    )],
                    [book.key().clone()].into_iter().collect(),
                ),
                SyncState::new("books-1"),
            ),
            cursor_recovered: false,
        })
    }

    async fn sync_contacts(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        Ok(ContactSourceSync::Available {
            sync: ScopeSync::new(
                SyncUpdate::snapshot(
                    vec![self.card.clone()],
                    [self.card.id.key().clone()].into_iter().collect(),
                ),
                SyncState::new("cards-1"),
            ),
            cursor_recovered: false,
        })
    }

    async fn fetch_contact_photo(
        &self,
        _account: &AccountId,
        card: &ContactCard,
        _media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        // A source knows only its own ids, and Graph refuses another's as malformed.
        if card.id != self.card.id {
            return Err(ProviderError::permanent("Id is malformed."));
        }
        Ok(self
            .photo
            .clone()
            .map(|bytes| ContactPhoto::new(bytes, Some("image/png".into()), "rev-1")))
    }
}

/// A second source on the same account, a directory: the same card, in a scope of its own.
pub(super) struct Directory(pub(super) FakeContacts);

impl Provider for Directory {
    fn connection_info(&self) -> ConnectionInfo {
        self.0.connection_info()
    }
}

impl engine_api::MailboxWrites for Directory {}
impl CalendarWrites for Directory {}

#[async_trait]
impl ContactsProvider for Directory {
    fn address_book_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GraphContactFolderList {
            account: account.clone(),
        }
    }

    fn contact_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::GraphDirectoryUsers {
            account: account.clone(),
        }
    }

    async fn sync_address_books(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        self.0.sync_address_books(account, cursor).await
    }

    async fn sync_contacts(
        &self,
        account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        self.0.sync_contacts(account, cursor).await
    }

    async fn fetch_contact_photo(
        &self,
        account: &AccountId,
        card: &ContactCard,
        media: &ContactResource,
    ) -> ProviderResult<Option<ContactPhoto>> {
        self.0.fetch_contact_photo(account, card, media).await
    }
}

/// A card for `email`, advertising a photo resource.
pub(super) fn card_with_photo(email: &str) -> ContactCard {
    let mut card = ContactCard::new(
        ContactId::try_from("c1").unwrap(),
        Memberships::of_one(AddressBookId::try_from("book").unwrap()),
    );
    card.name = Some(ContactName {
        full: Some("Ada Lovelace".to_owned()),
        ..ContactName::default()
    });
    let mut emails = BTreeMap::new();
    emails.insert(
        PropertyId::new("e1").unwrap(),
        ContactProperty::new(ContactEmail::new(email)),
    );
    card.emails = emails;
    let mut media = BTreeMap::new();
    media.insert(
        PropertyId::new("photo").unwrap(),
        ContactProperty::new(ContactResource {
            uri: "https://contacts.example/ada".into(),
            kind: Some("photo".into()),
            fingerprint: Some("photo-1".into()),
            ..ContactResource::default()
        }),
    );
    card.media = media;
    card
}

pub(super) fn app(contacts: FakeContacts, surfaces: &Arc<Mutex<Vec<Surface>>>) -> App<MailOnly> {
    app_with_sources(vec![Box::new(contacts)], surfaces)
}

/// The app over an account whose contacts come from every one of `sources`.
pub(super) fn app_with_sources(
    sources: Vec<Box<dyn ContactsProvider>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<MailOnly> {
    App::new(
        Engine::open_in_memory().unwrap(),
        vec![Account {
            id: AccountId::try_from("work").unwrap(),
            providers: vec![MailOnly].into(),
            calendar_provider: None,
            contact_providers: sources,
            identity: EmailAddress::new("me@work.local"),
            dialled: true,
            uses_mail: true,
        }],
        TimeZoneInit {
            device_zone: TimeZoneId::utc(),
            prefs_path: None,
        },
        None,
        Arc::new(RecordingObserver {
            surfaces: Arc::clone(surfaces),
        }),
        Telemetry::off(None),
    )
}

/// A snapshot holding one row from `address`.
pub(super) fn snapshot_from(address: &str) -> MailboxListSnapshot {
    MailboxListSnapshot {
        rows: vec![SnapshotRow::Flat(FlatRow {
            account: "work".to_owned(),
            key: "m1".to_owned(),
            subject: "Quarterly report".to_owned(),
            from: "Ada Lovelace".to_owned(),
            from_address: address.to_owned(),
            avatar: avatar::resolve("Ada Lovelace", address, None),
            date: String::new(),
            unread: true,
            flagged: false,
            has_attachment: false,
            preview: String::new(),
            kind: mailcal_viewmodel::MessageKind::Received,
            folders: Vec::new(),
        })],
        ..MailboxListSnapshot::default()
    }
}

pub(super) fn image_path(snapshot: &MailboxListSnapshot) -> Option<String> {
    match snapshot.rows.first() {
        Some(SnapshotRow::Flat(row)) => row.avatar.image_path.clone(),
        _ => None,
    }
}
