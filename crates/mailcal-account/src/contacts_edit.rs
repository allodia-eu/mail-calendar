//! Contact-write builders: the glue that turns a client's edited form into the engine's
//! neutral [`ContactDraft`] and [`ContactPatch`].
//!
//! It lives here beside [`crate::calendar`] for the same reason: the app stays free of a
//! direct dependency on the provider crates, and the host submits the result through
//! `Engine::create_contact` / `Engine::patch_contact`.
//!
//! # The two rules worth reading before editing this file
//!
//! **A patch carries only what changed.** Every field it names is *replaced*, and a
//! replacement built from a form loses whatever the form cannot show: an address's
//! `TYPE=work`, an organisation's departments, whether a title was a `TITLE` or a `ROLE`.
//! So an unchanged field is left out entirely, and a changed one is built on top of the
//! property the card already carried rather than from nothing.
//!
//! **`N` is written only when there is a surname.** A card may legitimately carry a
//! formatted name and no structured one, and the form seeds such a card by putting the
//! whole name in the given-name field. Emitting components from that would file "Ada
//! Lovelace" as a first name with no surname, in every other client the user owns, on a
//! save that changed nothing.

use std::collections::BTreeMap;

use engine_core::{
    contact::{
        ContactCard, ContactDraft, ContactEmail, ContactField, ContactName, ContactPatch,
        ContactPhone, ContactProperty, FieldPatch, NameComponent, NameComponentKind, Organization,
        PropertyId, Title,
    },
    ids::{AddressBookId, ContactId},
    membership::Memberships,
};

use crate::AccountError;

/// The editable half of one contact card, as a client's form holds it.
///
/// Deliberately the same shape a contacts detail screen already shows, so nothing is
/// editable that the app will not display back afterwards. Postal addresses, notes,
/// anniversaries and photos are carried untouched through an edit; they are not in this
/// struct because no client draws them yet.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContactEdit {
    /// The given (first) name.
    ///
    /// A card with no structured name seeds its **whole** formatted name here, which is why
    /// the surname decides whether `N` is written at all (see the module docs).
    pub given_name: String,
    /// The surname (family name).
    pub surname: String,
    /// The organisation name. The only field a company contact needs.
    pub organization: String,
    /// The job title or role.
    pub title: String,
    /// The email addresses, in the order the form lists them. Blank entries are dropped.
    pub emails: Vec<String>,
    /// The phone numbers, in the order the form lists them. Blank entries are dropped.
    pub phones: Vec<String>,
}

impl ContactEdit {
    /// Reads a stored card back into the form a client edits.
    ///
    /// A card with no structured name puts its formatted name in [`Self::given_name`] whole:
    /// splitting it on a space would guess, and it would guess wrong on every name with a
    /// particle or two given names. The user can split it; this will not.
    #[must_use]
    pub fn from_card(card: &ContactCard) -> Self {
        let name = card.name.as_ref();
        let component = |kind: &NameComponentKind| {
            name.and_then(|name| {
                name.components
                    .iter()
                    .find(|component| &component.kind == kind)
                    .map(|component| component.value.clone())
            })
            .unwrap_or_default()
        };
        let surname = component(&NameComponentKind::Surname);
        let given = component(&NameComponentKind::Given);
        let given = if given.is_empty() && surname.is_empty() {
            card.display_name().unwrap_or_default()
        } else {
            given
        };
        Self {
            given_name: given,
            surname,
            organization: card
                .organizations
                .values()
                .next()
                .map(|entry| entry.value.name.clone())
                .unwrap_or_default(),
            title: card
                .titles
                .values()
                .next()
                .map(|entry| entry.value.name.clone())
                .unwrap_or_default(),
            emails: card
                .emails
                .values()
                .map(|entry| entry.value.address.clone())
                .collect(),
            phones: card
                .phones
                .values()
                .map(|entry| entry.value.number.clone())
                .collect(),
        }
    }

    /// The trimmed form, with blank list entries dropped.
    fn trimmed(&self) -> Self {
        let list = |values: &[String]| {
            values
                .iter()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .collect()
        };
        Self {
            given_name: self.given_name.trim().to_owned(),
            surname: self.surname.trim().to_owned(),
            organization: self.organization.trim().to_owned(),
            title: self.title.trim().to_owned(),
            emails: list(&self.emails),
            phones: list(&self.phones),
        }
    }

    /// The formatted name (`FN`) this edit produces: the two name parts, else the
    /// organisation, else the first address.
    ///
    /// The fallbacks matter because `FN` is what every client sorts and files by, and a card
    /// with none is a blank row. They mirror what the engine already does when it picks a
    /// person's display name, so a company contact reads as its company rather than as
    /// nothing.
    fn formatted_name(&self) -> String {
        let full = [self.given_name.as_str(), self.surname.as_str()]
            .iter()
            .filter(|part| !part.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        if !full.is_empty() {
            return full;
        }
        if !self.organization.is_empty() {
            return self.organization.clone();
        }
        self.emails.first().cloned().unwrap_or_default()
    }

    /// The structured name, or `None` when this edit has nothing to say about one.
    fn name(&self) -> Option<ContactName> {
        let full = self.formatted_name();
        if full.is_empty() {
            return None;
        }
        let mut components = Vec::new();
        // Only with a surname; see the module docs.
        if !self.surname.is_empty() {
            if !self.given_name.is_empty() {
                components.push(NameComponent::new(
                    NameComponentKind::Given,
                    self.given_name.clone(),
                ));
            }
            components.push(NameComponent::new(
                NameComponentKind::Surname,
                self.surname.clone(),
            ));
        }
        Some(ContactName {
            full: Some(full),
            components,
            ..ContactName::default()
        })
    }

    /// Rejects an edit that would file a card nobody could find again.
    ///
    /// The error text names no value: an address is content, and this string reaches the
    /// diagnostic log (`docs/logging.md`).
    fn validate(&self) -> Result<(), AccountError> {
        if self.given_name.is_empty()
            && self.surname.is_empty()
            && self.organization.is_empty()
            && self.emails.is_empty()
        {
            return Err(AccountError::ContactWrite(
                "a contact needs a name, an organisation or an email address".to_owned(),
            ));
        }
        if self.emails.iter().any(|email| !is_address_shaped(email)) {
            return Err(AccountError::ContactWrite(
                "one of the email addresses is not an address".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Whether a string is shaped like an email address.
///
/// A backstop, not a parser: the client validates for its own inline message, and the server
/// is the authority on what it will accept. What this refuses is the case that reaches the
/// server as a malformed card and comes back as an opaque 400: a value with no `@`, or with
/// nothing on one side of it.
fn is_address_shaped(value: &str) -> bool {
    value.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && !domain.is_empty() && !domain.contains('@') && !domain.starts_with('.')
    })
}

/// Builds the draft for a new card in `address_book`.
///
/// `uid` is the caller's globally unique id for the card; the engine mints the object key (a
/// CardDAV href, a JMAP id), so the caller never names one.
///
/// # Errors
///
/// Returns [`AccountError::ContactWrite`] when the edit names nothing to file the card under,
/// or carries a value that is not an email address.
pub fn build_contact_draft(
    address_book: AddressBookId,
    uid: &str,
    edit: &ContactEdit,
) -> Result<ContactDraft, AccountError> {
    let edit = edit.trimmed();
    edit.validate()?;
    let mut card = ContactCard::new(
        // Ignored on create; the receipt carries the id the server assigned.
        ContactId::try_from(uid).map_err(|err| AccountError::ContactWrite(err.to_string()))?,
        Memberships::of_one(address_book.clone()),
    );
    card.uid = Some(uid.to_owned());
    card.name = edit.name();
    card.emails = properties("email", &edit.emails, |value| ContactEmail::new(value));
    card.phones = properties("phone", &edit.phones, |value| ContactPhone {
        number: value.to_owned(),
        ..ContactPhone::default()
    });
    if !edit.organization.is_empty() {
        card.organizations.insert(
            property_id("organization"),
            ContactProperty::new(Organization {
                name: edit.organization.clone(),
                ..Organization::default()
            }),
        );
    }
    if !edit.title.is_empty() {
        card.titles.insert(
            property_id("title"),
            ContactProperty::new(Title {
                name: edit.title.clone(),
                ..Title::default()
            }),
        );
    }
    Ok(ContactDraft { address_book, card })
}

/// Builds the patch that turns `base` into `edit`.
///
/// **Only the fields that differ are named**, and each is rebuilt on top of the property the
/// card already carried, so an edit to a phone number cannot quietly strip an address's
/// `TYPE=work` or an organisation's departments (see the module docs).
///
/// An edit that changes nothing yields an empty patch. That is a real outcome, not a bug: the
/// caller decides whether to send it, and sending one would rewrite the card for no reason.
///
/// # Errors
///
/// Returns [`AccountError::ContactWrite`] on the same terms as [`build_contact_draft`].
pub fn build_contact_patch(
    base: &ContactCard,
    edit: &ContactEdit,
) -> Result<ContactPatch, AccountError> {
    let edit = edit.trimmed();
    edit.validate()?;
    let mut patch = ContactPatch::default();
    let current = ContactEdit::from_card(base).trimmed();

    if edit.given_name != current.given_name || edit.surname != current.surname {
        let name = patched_name(base.name.as_ref(), &edit);
        patch.fields.insert(
            ContactField::Name,
            FieldPatch::Set(
                serde_json::to_value(name)
                    .map_err(|err| AccountError::ContactWrite(err.to_string()))?,
            ),
        );
    }
    if edit.emails != current.emails {
        let values = rebuilt(
            &base.emails,
            &edit.emails,
            "email",
            |entry| entry.address.clone(),
            |value| ContactEmail::new(value),
        );
        set(&mut patch, ContactField::Emails, &values)?;
    }
    if edit.phones != current.phones {
        let values = rebuilt(
            &base.phones,
            &edit.phones,
            "phone",
            |entry| entry.number.clone(),
            |value| ContactPhone {
                number: value.to_owned(),
                ..ContactPhone::default()
            },
        );
        set(&mut patch, ContactField::Phones, &values)?;
    }
    if edit.organization != current.organization {
        let mut values = BTreeMap::new();
        if !edit.organization.is_empty() {
            // On the base entry, so a rename keeps the departments and the provider
            // extensions the form never saw.
            let (id, mut entry) = single(&base.organizations, "organization");
            entry.value.name.clone_from(&edit.organization);
            values.insert(id, entry);
        }
        set(&mut patch, ContactField::Organizations, &values)?;
    }
    if edit.title != current.title {
        let mut values = BTreeMap::new();
        if !edit.title.is_empty() {
            // On the base entry, so a value the card recorded as a `ROLE` stays one rather
            // than being promoted to a job title.
            let (id, mut entry) = single(&base.titles, "title");
            entry.value.name.clone_from(&edit.title);
            values.insert(id, entry);
        }
        set(&mut patch, ContactField::Titles, &values)?;
    }
    Ok(patch)
}

/// The structured name this edit writes, built on top of the one the card carried.
///
/// The form owns two components and nothing else, so a prefix, a middle name, a suffix, the
/// sort keys and the phonetic system all survive: correcting a surname is not a request to
/// drop "Dr." and a middle name from every other client the user owns. The two the form does
/// own are replaced **in place**, keeping the reading order the card recorded.
///
/// The **formatted** name follows the same rule, so a card whose components outnumber the
/// form's keeps them in the one string most clients actually show. It is assembled by
/// `ContactName::display`, the engine's own rule, rather than by a second one here. The
/// exception is a name the form holds *unstructured*: without a surname it emits no components
/// at all, so there is nothing to assemble from and the typed text is the whole name.
fn patched_name(base: Option<&ContactName>, edit: &ContactEdit) -> ContactName {
    let built = edit.name().unwrap_or_default();
    let Some(base) = base else {
        return built;
    };
    let structured = !built.components.is_empty();
    let mut name = base.clone();
    name.full = built.full;
    let mut fresh = built.components;
    // A kind the form emptied goes; see the module docs on why the form emits none at all
    // without a surname.
    name.components
        .retain(|component| !form_owned(&component.kind) || has_kind(&fresh, &component.kind));
    for component in &mut name.components {
        if let Some(index) = fresh
            .iter()
            .position(|replacement| replacement.kind == component.kind)
        {
            component.value = fresh.remove(index).value;
        }
    }
    // A kind the card did not carry has no place to be replaced into, so it goes on the end.
    name.components.extend(fresh);
    if structured {
        name.full = None;
        name.full = name.display();
    }
    name
}

/// Whether the editor's form is the authority on this component.
fn form_owned(kind: &NameComponentKind) -> bool {
    matches!(kind, NameComponentKind::Given | NameComponentKind::Surname)
}

/// Whether `components` carries one of `kind`.
fn has_kind(components: &[NameComponent], kind: &NameComponentKind) -> bool {
    components.iter().any(|component| &component.kind == kind)
}

/// Serialises one property map into the patch.
fn set<T: serde::Serialize>(
    patch: &mut ContactPatch,
    field: ContactField,
    values: &BTreeMap<PropertyId, ContactProperty<T>>,
) -> Result<(), AccountError> {
    patch
        .set_properties(field, values)
        .map_err(|err| AccountError::ContactWrite(err.to_string()))
}

/// A fresh property map from a list of values.
fn properties<T>(
    prefix: &str,
    values: &[String],
    build: impl Fn(&str) -> T,
) -> BTreeMap<PropertyId, ContactProperty<T>> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            (
                ordered_id(prefix, index),
                ContactProperty::new(build(value)),
            )
        })
        .collect()
}

/// Rebuilds a property map for `values`, keeping the metadata each value already had.
///
/// **The ids are minted fresh in the form's order**, because a property map is a `BTreeMap`
/// and the map's order is what the card's first address is read from: keeping the old ids
/// would let a reordering in the form come back reordered again by the id, and the person's
/// primary address (which the avatar and the list row are keyed on) would not be the one at
/// the top of the editor.
///
/// The metadata is matched by **value**, not by position: a user who deletes the first of
/// three addresses has moved the other two up, and matching by position would hand each of
/// them the previous one's contexts and preference.
fn rebuilt<T: Clone>(
    base: &BTreeMap<PropertyId, ContactProperty<T>>,
    values: &[String],
    prefix: &str,
    read: impl Fn(&T) -> String,
    build: impl Fn(&str) -> T,
) -> BTreeMap<PropertyId, ContactProperty<T>> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let entry = base
                .values()
                .find(|entry| &read(&entry.value) == value)
                .cloned()
                .unwrap_or_else(|| ContactProperty::new(build(value)));
            (ordered_id(prefix, index), entry)
        })
        .collect()
}

/// A property id whose lexical order is the list's order.
///
/// Zero-padded because a `BTreeMap` sorts its keys as text: `email-10` sits between `email-1`
/// and `email-2`, so an unpadded id reorders a contact with ten addresses.
fn ordered_id(prefix: &str, index: usize) -> PropertyId {
    property_id(&format!("{prefix}-{:03}", index + 1))
}

/// The card's first entry of a single-valued field, or a fresh one under `fallback`.
fn single<T: Clone + Default>(
    base: &BTreeMap<PropertyId, ContactProperty<T>>,
    fallback: &str,
) -> (PropertyId, ContactProperty<T>) {
    base.iter().next().map_or_else(
        || (property_id(fallback), ContactProperty::new(T::default())),
        |(id, entry)| (id.clone(), entry.clone()),
    )
}

/// A property id built from text this module controls, so it is never empty.
fn property_id(value: &str) -> PropertyId {
    PropertyId::new(value).expect("a non-empty generated property id")
}

#[cfg(test)]
#[path = "contacts_edit_tests.rs"]
mod tests;
