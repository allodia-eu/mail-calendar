//! The contacts view-model: an alphabetical snapshot of **unified people**, and the detail
//! one of them expands to.
//!
//! A pure projection over the engine's [`Person`], mirroring [`crate::calendar::agenda`] for
//! events; state lives in the engine, grouping and ordering live here, the host renders.
//!
//! # The one thing this module does not decide
//!
//! **Which source cards are the same person is the engine's answer, not ours.** The engine
//! joins them on **shared canonical email only**; never on names, because two different
//! people commonly share one. So this module never compares, matches, or merges: it reads a
//! `Person` that is already unified and reports *how* it was unified, so a merged row can say
//! so on screen rather than silently presenting three cards as one.
//!
//! That disclosure is a product rule (`docs/contacts.md`), which is why
//! [`ContactRow::account_count`] exists at all: a user who sees one row where they filed two
//! contacts must be able to find out why.

use std::collections::BTreeSet;

use engine_api::{ContactKind, Person, PersonSource, PersonSourceId};

/// An immutable contacts snapshot for a host to render, ordered for an A–Z list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactsSnapshot {
    /// The people, ordered by display name (case-insensitively), then by id so the order
    /// is total and stable across rebuilds.
    pub rows: Vec<ContactRow>,
}

/// One contacts-list row: a unified person reduced to what a list cell shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactRow {
    /// The person's stable store-local id, as a string for the FFI. Resolves through the
    /// engine's alias table, so a row the host is still holding after a merge still opens.
    pub id: String,
    /// The deterministically selected display name, **empty** when every source card is
    /// nameless (a card can legitimately have an email and no name). The placeholder is the
    /// client's to supply: the core has no locale, so anything it put here could only ever be
    /// English (`docs/contacts.md` §2).
    pub display_name: String,
    /// The address a list cell shows under the name: the person's first canonical email,
    /// empty when the person has none (a phone-only contact).
    pub primary_email: String,
    /// The letter this row files under in an A–Z list: the display name's first character,
    /// uppercased and folded to its base Latin letter (so "Émile" files under `E`). `#` when
    /// the name does not start with a letter, so digits and symbols collect in one section
    /// rather than each minting their own.
    pub section: String,
    /// The person's monogram, colour and photo ([`docs/avatars.md`](../../../docs/avatars.md)).
    pub avatar: Avatar,
    /// How many **distinct accounts** contributed a card to this person. `1` for an ordinary
    /// contact; `2` or more means the row is a merge, and the host says so (see the module
    /// docs). Deliberately accounts and not source *cards*: two books in one account reading
    /// "in 2 accounts" would be a lie.
    pub account_count: u32,
}

/// The detail view of one unified person: every value, and which accounts supplied it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactDetail {
    /// The person's stable store-local id, as a string for the FFI.
    pub id: String,
    /// The display name, empty under the same rule the row follows.
    pub display_name: String,
    /// The person's monogram, colour and photo, matching the row's.
    pub avatar: Avatar,
    /// Every canonical email, with the accounts that carry it.
    pub emails: Vec<ContactValue>,
    /// Every phone number, with the accounts that carry it.
    pub phones: Vec<ContactValue>,
    /// Every organisation name, with the accounts that carry it.
    pub organizations: Vec<ContactValue>,
    /// Every job title/role, with the accounts that carry it.
    pub titles: Vec<ContactValue>,
    /// The distinct accounts this person was assembled from, sorted. The detail screen
    /// lists these under "Also in", which is the *explanation* of a merged row.
    pub accounts: Vec<String>,
    /// The source cards behind this person that can be edited **where they live**.
    ///
    /// An edit names a card, never a person, and this is why: a person is several cards, and
    /// writing the values on screen back without choosing one would file the work account's
    /// details in the personal account's address book. Empty for a person every source of
    /// which is read-only (a directory, a suggested-contacts source, a shared book the
    /// account may only read), and a client shows no edit affordance then rather than one
    /// that fails on press. More than one is a merge the user has to resolve: the client asks
    /// which card, naming the account.
    pub editable_cards: Vec<ContactCardRef>,
}

/// One source card, named the way a write names it: the account, then the card.
///
/// A card id is unique only within its account, which is why neither half travels alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactCardRef {
    /// The account holding the card.
    pub account: String,
    /// The card's provider id.
    pub card: String,
}

/// One value on a contact, and the accounts it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactValue {
    /// The value itself (an email address, a phone number, an organisation, a title).
    pub value: String,
    /// The distinct accounts carrying it, sorted. A value present in two accounts lists
    /// both, so the detail view can show where each fact is filed.
    pub accounts: Vec<String>,
}

/// Builds the alphabetical contacts snapshot from the engine's unified people.
///
/// **Group cards are excluded.** A vCard `KIND:GROUP` ("Harness Friends", "Team") is a
/// container, not a person: it has no address, so it renders as a row with a blank second line
/// that a user taps expecting someone and finds nothing. The engine derives people from every
/// card kind; filtering is this layer's job. Organisations are **not** excluded: a company with
/// an address is a legitimate contact.
#[must_use]
pub fn build(people: &[Person]) -> ContactsSnapshot {
    let mut rows: Vec<ContactRow> = people
        .iter()
        .filter(|person| !is_group(person))
        .map(row)
        .collect();
    // Case- and diacritic-insensitive by name, then by id: the id tiebreak is what makes the
    // order total, so two people with the same display name never swap places between rebuilds.
    rows.sort_by(|left, right| {
        sort_key(&left.display_name)
            .cmp(&sort_key(&right.display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
    ContactsSnapshot { rows }
}

/// Whether every source card behind this person is a group.
///
/// Checked across *all* the person's kinds rather than any single one: a person whose sources
/// include a real individual card is a person, whatever else joined to it.
fn is_group(person: &Person) -> bool {
    !person.kinds.is_empty() && person.kinds.iter().all(|kind| *kind == ContactKind::Group)
}

/// Projects one person into a list row.
fn row(person: &Person) -> ContactRow {
    let display_name = display_name(person);
    let primary_email = person
        .emails
        .first()
        .map(|email| email.value.as_str().to_owned())
        .unwrap_or_default();
    let avatar = avatar::resolve(&display_name, &primary_email, None);
    ContactRow {
        id: person.id.get().to_string(),
        section: section(&display_name),
        avatar,
        primary_email,
        display_name,
        account_count: u32::try_from(accounts(&person.sources).len()).unwrap_or(u32::MAX),
    }
}

/// Projects one person into the detail view.
///
/// `sources` is the engine's list of the live cards behind this person, which carries the one
/// thing the `Person` cannot: which of them the user may edit. It is read separately rather
/// than inferred from [`Person::is_writable`], which says only that *some* source is writable
/// and so cannot name the card an edit would go to.
#[must_use]
pub fn detail(person: &Person, sources: &[PersonSource]) -> ContactDetail {
    let display_name = display_name(person);
    let avatar = avatar::resolve(
        &display_name,
        person
            .emails
            .first()
            .map_or("", |email| email.value.as_str()),
        None,
    );
    ContactDetail {
        id: person.id.get().to_string(),
        avatar,
        display_name,
        emails: person
            .emails
            .iter()
            .map(|email| value(email.value.as_str(), &email.sources))
            .collect(),
        phones: person
            .phones
            .iter()
            .map(|phone| value(&phone.value, &phone.sources))
            .collect(),
        organizations: person
            .organizations
            .iter()
            .map(|organization| value(&organization.value, &organization.sources))
            .collect(),
        titles: person
            .titles
            .iter()
            .map(|title| value(&title.value, &title.sources))
            .collect(),
        accounts: accounts(&person.sources),
        editable_cards: sources
            .iter()
            .filter(|source| source.writable)
            .map(|source| ContactCardRef {
                account: source.id.account.as_str().to_owned(),
                card: source.id.contact.as_str().to_owned(),
            })
            .collect(),
    }
}

/// Pairs a value with the distinct accounts that carry it.
fn value(value: &str, sources: &BTreeSet<PersonSourceId>) -> ContactValue {
    ContactValue {
        value: value.to_owned(),
        accounts: accounts(sources),
    }
}

/// The distinct account ids behind a set of source cards, sorted and deduplicated.
///
/// Source ids are `(account, contact)`, so a person with two cards in **one** account has
/// two sources but one account. Everything user-facing counts accounts, not cards.
fn accounts(sources: &BTreeSet<PersonSourceId>) -> Vec<String> {
    sources
        .iter()
        .map(|source| source.account.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The person's name, **empty** when every source card is nameless.
///
/// The engine reports that case as `None` for the same reason this reports it as empty: the
/// core has no runtime locale facility, so any text put here would be English on a Dutch
/// device, and a client cannot detect a placeholder it did not choose. Empty is a signal a
/// client *can* act on; it substitutes its own localised string (`docs/contacts.md` §2). The
/// FFI record is a plain `String`, so `None` flattens to `""` rather than widening the
/// binding with an optional every host would then have to unwrap.
fn display_name(person: &Person) -> String {
    person
        .display_name
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The A–Z section a name files under; `#` for anything not starting with a letter.
fn section(display_name: &str) -> String {
    display_name
        .chars()
        .next()
        .and_then(base_letter)
        .map_or_else(|| "#".to_owned(), |letter| letter.to_string())
}

/// The ordering key: the name folded to base letters and lowercased.
///
/// Without the fold, the raw string's code points decide, and every accented letter is
/// numerically greater than `z`: so "Émile" sorts after *every* ASCII name rather than
/// between "Emil" and "Emma". Non-letters pass through unchanged, so digits and symbols keep
/// their natural order.
fn sort_key(display_name: &str) -> String {
    display_name
        .chars()
        .map(|character| base_letter(character).unwrap_or(character))
        .collect::<String>()
        .to_lowercase()
}

/// The base ASCII letter a character files under, or `None` when it is not a Latin letter.
///
/// Latin diacritics fold to their base letter, because that is where a reader looks: "Émile"
/// belongs next to "Emma", under **E**. Treating only `is_ascii_alphabetic` as a letter files
/// every Dutch, German, French, Scandinavian or Polish name under `#`: a second `#` section
/// past Z, since those code points also sort after every ASCII name, which is both halves of
/// the same bug and contradicts this module's own contract ("`#` for anything not starting
/// with a **letter**").
///
/// The table covers Latin-1 Supplement and Latin Extended-A, which is what a European address
/// book holds. Anything outside it (Greek, Cyrillic, Hebrew, CJK) files under `#` rather
/// than being folded to a Latin letter it is not; giving those scripts their own sections is a
/// feature, not a fold (`docs/contacts.md`, Known gaps).
fn base_letter(character: char) -> Option<char> {
    // `to_uppercase` first, so each arm below lists only the uppercase form, and so `ß`,
    // whose uppercase is "SS", folds to `S` without an arm of its own.
    let upper = character.to_uppercase().next()?;
    Some(match upper {
        'A'..='Z' => upper,
        'À'..='Å' | 'Æ' | 'Ā' | 'Ă' | 'Ą' => 'A',
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => 'C',
        'Ð' | 'Ď' | 'Đ' => 'D',
        'È'..='Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'E',
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => 'G',
        'Ĥ' | 'Ħ' => 'H',
        'Ì'..='Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' | 'Ĳ' => 'I',
        'Ĵ' => 'J',
        'Ķ' => 'K',
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => 'L',
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' | 'Ŋ' => 'N',
        'Ò'..='Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' | 'Œ' => 'O',
        'Ŕ' | 'Ŗ' | 'Ř' => 'R',
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => 'S',
        'Þ' | 'Ţ' | 'Ť' | 'Ŧ' => 'T',
        'Ù'..='Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => 'U',
        'Ŵ' => 'W',
        'Ý' | 'Ŷ' | 'Ÿ' => 'Y',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        _ => return None,
    })
}

// The monogram derivation moved to `crate::avatar`, which the mail rows use too. Two
// surfaces deriving letters separately is a disagreement waiting to happen: the same person
// would be `AL` in one list and `A` in the other.
use crate::avatar::{self, Avatar};

#[cfg(test)]
#[path = "contacts_tests.rs"]
mod tests;
