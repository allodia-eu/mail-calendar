//! The showcase/demo contacts source: a canned address book, and the people it holds.
//!
//! Seeded so a screenshot run and `--account demo` have a populated Contacts list without a
//! server. The **primary** and **secondary** showcase accounts deliberately share one person
//! (`iris.jansen@example.eu`), so the merged-row affordance ("in 2 accounts") appears in the
//! showcase rather than only against the live harness. A screenshot that never shows the merge
//! cannot catch the merge rendering wrong.
//!
//! Names are proper nouns and are **not** localised: the same people appear in every locale, so
//! this needs no `en`/`nl` split (unlike the mail and calendar seeds, whose subjects are prose).

use std::collections::BTreeMap;

use engine_api::{AccountId, ContactCard, ContactSourceClass, SyncScope};
use engine_core::{
    contact::{
        AddressBook, ContactEmail, ContactName, ContactPhone, ContactProperty, Organization,
        PropertyId, Title,
    },
    ids::{AddressBookId, ContactId},
    membership::Memberships,
    sync::{SyncState, SyncUpdate},
};
use engine_provider::{
    Capabilities, ConnectionInfo, ContactSourceSync, ContactsProvider, Provider, ProviderResult,
    ScopeSync,
};

/// The single address book every showcase account exposes.
const BOOK: &str = "showcase-contacts";

/// A canned contacts source over a fixed set of cards.
pub(crate) struct ShowcaseContactsProvider {
    caps: Capabilities,
    cards: Vec<ContactCard>,
}

impl ShowcaseContactsProvider {
    pub(crate) fn new(cards: Vec<ContactCard>) -> Self {
        Self {
            caps: Capabilities::none().with_contacts(),
            cards,
        }
    }

    fn book() -> AddressBookId {
        AddressBookId::try_from(BOOK).expect("valid address book id")
    }
}

impl core::fmt::Debug for ShowcaseContactsProvider {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ShowcaseContactsProvider")
            .field("cards", &self.cards.len())
            .finish()
    }
}

#[async_trait::async_trait]
impl Provider for ShowcaseContactsProvider {
    fn connection_info(&self) -> ConnectionInfo {
        ConnectionInfo::new(self.caps)
    }
}

#[async_trait::async_trait]
impl ContactsProvider for ShowcaseContactsProvider {
    fn address_book_scope(&self, account: &AccountId) -> SyncScope {
        SyncScope::JmapType {
            account: account.clone(),
            data_type: engine_core::sync::JmapDataType::AddressBook,
        }
    }

    async fn sync_address_books(
        &self,
        _account: &AccountId,
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<AddressBook>> {
        if cursor.is_some() {
            return Ok(ContactSourceSync::Available {
                sync: ScopeSync::new(
                    SyncUpdate::delta(Vec::new(), Vec::new()),
                    SyncState::new("books-2"),
                ),
                cursor_recovered: false,
            });
        }
        let book = Self::book();
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
        cursor: Option<&SyncState>,
    ) -> ProviderResult<ContactSourceSync<ContactCard>> {
        if cursor.is_some() {
            return Ok(ContactSourceSync::Available {
                sync: ScopeSync::new(
                    SyncUpdate::delta(Vec::new(), Vec::new()),
                    SyncState::new("cards-2"),
                ),
                cursor_recovered: false,
            });
        }
        Ok(ContactSourceSync::Available {
            sync: ScopeSync::new(
                SyncUpdate::snapshot(
                    self.cards.clone(),
                    self.cards
                        .iter()
                        .map(|card| card.id.key().clone())
                        .collect(),
                ),
                SyncState::new("cards-1"),
            ),
            cursor_recovered: false,
        })
    }
}

/// One seeded card. `organization`/`title`/`phone` are optional so the list shows a realistic
/// mix rather than every row carrying every field.
fn card(
    id: &str,
    name: &str,
    email: &str,
    phone: Option<&str>,
    organization: Option<(&str, &str)>,
) -> ContactCard {
    let book = AddressBookId::try_from(BOOK).expect("valid address book id");
    let mut card = ContactCard::new(
        ContactId::try_from(id).expect("valid contact id"),
        Memberships::of_one(book),
    );
    card.source_class = ContactSourceClass::Personal;
    card.name = Some(ContactName {
        full: Some(name.to_owned()),
        ..ContactName::default()
    });
    card.emails = one(ContactEmail::new(email));
    if let Some(phone) = phone {
        card.phones = one(ContactPhone {
            number: phone.to_owned(),
            ..ContactPhone::default()
        });
    }
    if let Some((org, title)) = organization {
        card.organizations = one(Organization {
            name: org.to_owned(),
            ..Organization::default()
        });
        card.titles = one(Title {
            name: title.to_owned(),
            ..Title::default()
        });
    }
    card
}

/// Wraps a single value as the one property of its kind.
fn one<T>(value: T) -> BTreeMap<PropertyId, ContactProperty<T>> {
    let mut map = BTreeMap::new();
    map.insert(
        PropertyId::new("p1").expect("valid property id"),
        ContactProperty::new(value),
    );
    map
}

/// The primary showcase account's contacts.
pub(crate) fn primary_contacts() -> Vec<ContactCard> {
    vec![
        card(
            "sc-1",
            "Iris Jansen",
            "iris.jansen@example.eu",
            Some("+31 20 123 4567"),
            Some(("Meridiaan Bouw", "Projectleider")),
        ),
        card(
            "sc-2",
            "Ahmed El Amrani",
            "ahmed.elamrani@example.eu",
            Some("+31 6 2233 4455"),
            Some(("Noordlicht Studio", "Art Director")),
        ),
        card("sc-3", "Sofie Vermeulen", "sofie@example.eu", None, None),
        card(
            "sc-4",
            "Thomas Bakker",
            "t.bakker@example.eu",
            Some("+31 30 987 6543"),
            Some(("Bakker & Zonen", "Eigenaar")),
        ),
        card("sc-5", "Lena Novak", "lena.novak@example.eu", None, None),
    ]
}

/// The secondary showcase account's contacts.
///
/// `sc-6` is **Iris Jansen again, at the same address**: a different card id in a different
/// account. The engine joins the two on that shared address, so the showcase list shows five
/// people rather than seven, with Iris marked as being in two accounts.
pub(crate) fn secondary_contacts() -> Vec<ContactCard> {
    vec![
        card(
            "sc-6",
            "Iris Jansen",
            "iris.jansen@example.eu",
            None,
            Some(("Meridiaan Bouw", "Projectleider")),
        ),
        card(
            "sc-7",
            "Pieter de Groot",
            "pieter.degroot@example.eu",
            Some("+31 10 555 0199"),
            Some(("Havenbedrijf", "Planner")),
        ),
    ]
}
