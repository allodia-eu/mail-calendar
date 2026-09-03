//! The fakes every contacts test runs against: a mail-only provider so an account can exist,
//! a contacts adapter serving one address book, and the app that holds them.
//!
//! Split out of `tests_contacts.rs` to keep both files under the 500-line limit; the write
//! tests next door use the same adapter, which is the point: a create has to be visible to
//! the *sync* that follows it, or every write reads as a silent no-op.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use engine_api::{AccountId, EmailAddress, Engine, TimeZoneId};
use engine_core::{
    contact::{
        AddressBook, ContactCard, ContactDraft, ContactEmail, ContactName, ContactPatch,
        ContactProperty, ContactSourceClass, PropertyId,
    },
    ids::{AddressBookId, ContactId},
    membership::Memberships,
    sync::{SyncState, SyncUpdate},
};
use engine_provider::{
    Capabilities, ConnectionInfo, ContactDestination, ContactSourceSync, ContactWriteReceipt,
    ContactsProvider, Provider, ProviderError, ProviderResult, ScopeSync, WriteGuard,
};

use crate::{Account, App, AppObserver, Surface, Telemetry, TimeZoneInit};

/// Records the surfaces the app signals.
pub(crate) struct RecordingObserver {
    pub(crate) surfaces: Arc<Mutex<Vec<Surface>>>,
}

impl AppObserver for RecordingObserver {
    fn surface_changed(&self, surface: Surface) {
        self.surfaces.lock().unwrap().push(surface);
    }
}

/// A minimal mail provider, so an account can exist. Contacts ride on a separate adapter.
pub(crate) struct MailOnly;

#[async_trait]
impl Provider for MailOnly {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_mail())
    }
}

/// A contacts adapter serving one address book of canned cards.
///
/// `fails` makes every sync error, which is how the "one bad source must not cost the user
/// the sources that worked" behaviour is exercised. `writable` is what a client's "save
/// to…" picker is built from, and a source that does not advertise a destination is one
/// contacts come *from*: the write tests next door depend on both answers.
///
/// The cards are behind a mutex because a write mutates them: a create appends, a patch
/// replaces, and the engine re-reads the card it just wrote through `fetch_contact`. A fake
/// that answered the pre-write list would report every write as a silent no-op.
pub(crate) struct FakeContacts {
    book: AddressBookId,
    cards: Mutex<Vec<ContactCard>>,
    fails: bool,
    writable: bool,
    /// Every create and patch this adapter was asked for, in order.
    ///
    /// Shared rather than owned, so a test can read it while the app owns the adapter: the
    /// adapter is moved into an `Account`, and a `ContactsProvider` cannot be handed out
    /// behind an `Arc` (a foreign trait on a foreign wrapper).
    writes: WriteLog,
}

/// A shared record of what an adapter was asked to write.
#[derive(Clone, Default)]
pub(crate) struct WriteLog(Arc<Mutex<Vec<ContactWriteRecord>>>);

impl WriteLog {
    pub(crate) fn entries(&self) -> Vec<ContactWriteRecord> {
        self.0.lock().unwrap().clone()
    }

    fn push(&self, record: ContactWriteRecord) {
        self.0.lock().unwrap().push(record);
    }
}

/// One write the fake was asked to perform.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ContactWriteRecord {
    Created(Box<ContactCard>),
    Patched(ContactId, Box<ContactPatch>),
}

impl FakeContacts {
    pub(crate) fn new(book: &str, cards: Vec<ContactCard>) -> Self {
        Self {
            book: AddressBookId::try_from(book).unwrap(),
            cards: Mutex::new(cards),
            fails: false,
            writable: false,
            writes: WriteLog::default(),
        }
    }

    pub(crate) fn failing(book: &str) -> Self {
        Self {
            book: AddressBookId::try_from(book).unwrap(),
            cards: Mutex::new(Vec::new()),
            fails: true,
            writable: false,
            writes: WriteLog::default(),
        }
    }

    /// The same adapter, advertising a writable destination.
    pub(crate) fn writable(self) -> Self {
        Self {
            writable: true,
            ..self
        }
    }

    /// The same adapter, recording every write into `log`.
    pub(crate) fn recording(self, log: &WriteLog) -> Self {
        Self {
            writes: log.clone(),
            ..self
        }
    }
}

#[async_trait]
impl Provider for FakeContacts {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(Capabilities::none().with_contacts())
    }
}

#[async_trait]
impl ContactsProvider for FakeContacts {
    fn contact_destination(&self) -> Option<ContactDestination> {
        self.writable.then(|| ContactDestination {
            address_book: self.book.clone(),
            source_class: ContactSourceClass::Personal,
            writable: true,
            write_guard: Some(WriteGuard::Absent),
            supported_fields: engine_core::contact::ContactFieldSet::from_fields([
                engine_core::contact::ContactField::Kind,
                engine_core::contact::ContactField::Name,
                engine_core::contact::ContactField::Emails,
                engine_core::contact::ContactField::Phones,
                engine_core::contact::ContactField::Organizations,
                engine_core::contact::ContactField::Titles,
            ]),
        })
    }

    async fn fetch_contact(
        &self,
        _account: &AccountId,
        contact: &ContactId,
    ) -> ProviderResult<ContactCard> {
        self.cards
            .lock()
            .unwrap()
            .iter()
            .find(|card| &card.id == contact)
            .cloned()
            .ok_or_else(|| ProviderError::invalid_state("no such card"))
    }

    async fn create_contact(
        &self,
        _account: &AccountId,
        draft: &ContactDraft,
    ) -> ProviderResult<ContactWriteReceipt> {
        let id =
            ContactId::try_from(format!("created-{}", self.cards.lock().unwrap().len()).as_str())
                .expect("a non-empty generated id");
        let mut card = draft.card.clone();
        card.id = id.clone();
        card.is_writable = true;
        self.writes
            .push(ContactWriteRecord::Created(Box::new(card.clone())));
        self.cards.lock().unwrap().push(card);
        Ok(ContactWriteReceipt::new(id))
    }

    async fn patch_contact(
        &self,
        _account: &AccountId,
        base: &ContactCard,
        patch: &ContactPatch,
    ) -> ProviderResult<ContactWriteReceipt> {
        self.writes.push(ContactWriteRecord::Patched(
            base.id.clone(),
            Box::new(patch.clone()),
        ));
        // Only the name is applied back, which is all the assertions read: a fake that
        // re-implemented the whole patch would be testing itself.
        let mut cards = self.cards.lock().unwrap();
        if let Some(card) = cards.iter_mut().find(|card| card.id == base.id)
            && let Some(engine_core::contact::FieldPatch::Set(name)) =
                patch.fields.get(&engine_core::contact::ContactField::Name)
        {
            card.name = serde_json::from_value(name.clone()).ok();
        }
        Ok(ContactWriteReceipt::new(base.id.clone()))
    }

    async fn sync_address_books(
        &self,
        _account: &AccountId,
        _cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        if self.fails {
            return Err(ProviderError::retryable("address book listing unavailable"));
        }
        Ok(ContactSourceSync::Available {
            sync: ScopeSync::new(
                SyncUpdate::snapshot(
                    vec![AddressBook::new(
                        self.book.clone(),
                        "Personal",
                        ContactSourceClass::Personal,
                    )],
                    [self.book.key().clone()].into_iter().collect(),
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
        if self.fails {
            return Err(ProviderError::retryable("card sync unavailable"));
        }
        let cards = self.cards.lock().unwrap().clone();
        Ok(ContactSourceSync::Available {
            sync: ScopeSync::new(
                SyncUpdate::snapshot(
                    cards.clone(),
                    cards.iter().map(|card| card.id.key().clone()).collect(),
                ),
                SyncState::new("cards-1"),
            ),
            cursor_recovered: false,
        })
    }
}

/// A card with `name` and `email`, filed in `book`.
///
/// Writable, because a stored card's own flag is what decides whether the detail offers to
/// edit it; a read-only one is built by clearing it.
pub(crate) fn card(id: &str, book: &str, name: &str, email: &str) -> ContactCard {
    let mut card = ContactCard::new(
        ContactId::try_from(id).unwrap(),
        Memberships::of_one(AddressBookId::try_from(book).unwrap()),
    );
    card.name = Some(ContactName {
        full: Some(name.to_owned()),
        ..ContactName::default()
    });
    let mut emails = BTreeMap::new();
    emails.insert(
        PropertyId::new("e1").unwrap(),
        ContactProperty::new(ContactEmail::new(email)),
    );
    card.emails = emails;
    card.is_writable = true;
    card
}

/// An account with `id`, a mail provider, and the given contacts adapters.
pub(crate) fn account(id: &str, contacts: Vec<Box<dyn ContactsProvider>>) -> Account<MailOnly> {
    Account {
        id: AccountId::try_from(id).unwrap(),
        providers: vec![MailOnly],
        calendar_providers: Vec::new(),
        contact_providers: contacts,
        identity: EmailAddress::new(format!("me@{id}.local")),
    }
}

/// An in-memory app over `accounts`.
pub(crate) fn app(
    accounts: Vec<Account<MailOnly>>,
    surfaces: &Arc<Mutex<Vec<Surface>>>,
) -> App<MailOnly> {
    App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
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
