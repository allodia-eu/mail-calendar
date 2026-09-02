//! What a contact edit must and must not do to the card underneath it.

use engine_core::contact::{FieldPatch, NameComponent, NameComponentKind, OrganizationUnit};

use super::*;

fn book() -> AddressBookId {
    AddressBookId::try_from("/contacts/").expect("a static book id")
}

fn edit() -> ContactEdit {
    ContactEdit {
        given_name: "Ada".into(),
        surname: "Lovelace".into(),
        organization: "Analytical Engines".into(),
        title: "Mathematician".into(),
        emails: vec!["ada@example.test".into()],
        phones: vec!["+44 123".into()],
    }
}

/// A card as a sync would have stored it: property metadata the editor never shows, which an
/// edit to a different field must not take away.
fn stored() -> ContactCard {
    let mut card = ContactCard::new(
        ContactId::try_from("/contacts/ada.vcf").expect("a static card id"),
        Memberships::of_one(book()),
    );
    card.name = Some(ContactName {
        full: Some("Ada Lovelace".into()),
        components: vec![
            NameComponent::new(NameComponentKind::Given, "Ada"),
            NameComponent::new(NameComponentKind::Surname, "Lovelace"),
        ],
        ..ContactName::default()
    });
    card.emails.insert(
        property_id("email-001"),
        ContactProperty {
            contexts: ["work".to_owned()].into_iter().collect(),
            preference: Some(1),
            ..ContactProperty::new(ContactEmail::new("ada@example.test"))
        },
    );
    card.phones.insert(
        property_id("phone-001"),
        ContactProperty::new(ContactPhone {
            number: "+44 123".into(),
            features: ["mobile".to_owned()].into_iter().collect(),
        }),
    );
    card.organizations.insert(
        property_id("organization"),
        ContactProperty::new(Organization {
            name: "Analytical Engines".into(),
            units: vec![OrganizationUnit {
                name: "Research".into(),
                ..OrganizationUnit::default()
            }],
            ..Organization::default()
        }),
    );
    card.titles.insert(
        property_id("title"),
        ContactProperty::new(Title {
            name: "Mathematician".into(),
            kind: Some("role".into()),
            ..Title::default()
        }),
    );
    card
}

#[test]
fn a_draft_carries_every_editable_field() {
    let draft = build_contact_draft(book(), "ada-uid", &edit()).expect("a valid draft");
    assert_eq!(draft.address_book, book());
    assert_eq!(draft.card.uid.as_deref(), Some("ada-uid"));
    assert_eq!(draft.card.display_name().as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        draft
            .card
            .emails
            .values()
            .map(|entry| entry.value.address.clone())
            .collect::<Vec<_>>(),
        vec!["ada@example.test".to_owned()]
    );
    assert_eq!(
        draft
            .card
            .phones
            .values()
            .next()
            .expect("the phone")
            .value
            .number,
        "+44 123"
    );
    assert_eq!(
        draft
            .card
            .organizations
            .values()
            .next()
            .expect("the organisation")
            .value
            .name,
        "Analytical Engines"
    );
    assert_eq!(
        draft
            .card
            .titles
            .values()
            .next()
            .expect("the title")
            .value
            .name,
        "Mathematician"
    );
}

/// A company contact needs no person's name, and must still file under something: the
/// organisation, else the first address. A card with a blank formatted name is a blank row in
/// every client's A-Z list.
#[test]
fn a_nameless_card_files_under_its_organization_then_its_address() {
    let company = ContactEdit {
        organization: "Analytical Engines".into(),
        ..ContactEdit::default()
    };
    let draft = build_contact_draft(book(), "org-uid", &company).expect("a valid draft");
    assert_eq!(
        draft.card.display_name().as_deref(),
        Some("Analytical Engines")
    );

    let address_only = ContactEdit {
        emails: vec!["ada@example.test".into()],
        ..ContactEdit::default()
    };
    let draft = build_contact_draft(book(), "mail-uid", &address_only).expect("a valid draft");
    assert_eq!(
        draft.card.display_name().as_deref(),
        Some("ada@example.test")
    );
}

/// `N` describes a *structured* name, and a card that never had one seeds the whole formatted
/// name into the given-name field. Writing components from that files "Ada Lovelace" as a
/// first name with no surname in every other client the user owns.
#[test]
fn no_surname_writes_no_structured_name() {
    let one_field = ContactEdit {
        given_name: "Ada Lovelace".into(),
        ..ContactEdit::default()
    };
    let draft = build_contact_draft(book(), "uid", &one_field).expect("a valid draft");
    let name = draft.card.name.expect("a formatted name");
    assert_eq!(name.full.as_deref(), Some("Ada Lovelace"));
    assert!(name.components.is_empty(), "{:?}", name.components);

    let draft = build_contact_draft(book(), "uid", &edit()).expect("a valid draft");
    let name = draft.card.name.expect("a formatted name");
    assert_eq!(
        name.components
            .iter()
            .map(|component| (component.kind.clone(), component.value.clone()))
            .collect::<Vec<_>>(),
        vec![
            (NameComponentKind::Given, "Ada".to_owned()),
            (NameComponentKind::Surname, "Lovelace".to_owned()),
        ]
    );
}

#[test]
fn an_edit_that_changed_nothing_patches_nothing() {
    let patch =
        build_contact_patch(&stored(), &ContactEdit::from_card(&stored())).expect("a valid patch");
    assert!(patch.fields.is_empty(), "{:?}", patch.fields.keys());
    assert!(patch.kind.is_none());
}

/// Every field a patch names is *replaced*, and a replacement built from the form alone loses
/// what the form cannot show. So an edit to one field must name only that one.
#[test]
fn a_patch_names_only_what_changed() {
    let mut changed = ContactEdit::from_card(&stored());
    changed.phones = vec!["+44 999".into()];
    let patch = build_contact_patch(&stored(), &changed).expect("a valid patch");
    assert_eq!(
        patch.fields.keys().copied().collect::<Vec<_>>(),
        vec![ContactField::Phones]
    );
}

/// The metadata a sync stored and the editor never showed: an address's contexts and
/// preference, an organisation's departments, whether a title was recorded as a `ROLE`.
#[test]
fn a_changed_field_keeps_the_metadata_the_form_cannot_show() {
    let mut changed = ContactEdit::from_card(&stored());
    changed.emails = vec!["ada@example.test".into(), "ada@second.test".into()];
    changed.organization = "Analytical Engines Ltd".into();
    changed.title = "Chief Mathematician".into();
    let patch = build_contact_patch(&stored(), &changed).expect("a valid patch");

    let FieldPatch::Set(emails) = &patch.fields[&ContactField::Emails] else {
        panic!("emails were cleared");
    };
    let emails: BTreeMap<PropertyId, ContactProperty<ContactEmail>> =
        serde_json::from_value(emails.clone()).expect("emails decode");
    let kept = emails
        .values()
        .find(|entry| entry.value.address == "ada@example.test")
        .expect("the address that did not change");
    assert!(kept.contexts.contains("work"));
    assert_eq!(kept.preference, Some(1));
    // The address the user added carries no invented metadata.
    let added = emails
        .values()
        .find(|entry| entry.value.address == "ada@second.test")
        .expect("the added address");
    assert!(added.contexts.is_empty());

    let FieldPatch::Set(organizations) = &patch.fields[&ContactField::Organizations] else {
        panic!("the organisation was cleared");
    };
    let organizations: BTreeMap<PropertyId, ContactProperty<Organization>> =
        serde_json::from_value(organizations.clone()).expect("organizations decode");
    let organization = organizations.values().next().expect("the organisation");
    assert_eq!(organization.value.name, "Analytical Engines Ltd");
    assert_eq!(
        organization
            .value
            .units
            .iter()
            .map(|unit| unit.name.clone())
            .collect::<Vec<_>>(),
        vec!["Research".to_owned()],
        "a rename must not delete the departments"
    );

    let FieldPatch::Set(titles) = &patch.fields[&ContactField::Titles] else {
        panic!("the title was cleared");
    };
    let titles: BTreeMap<PropertyId, ContactProperty<Title>> =
        serde_json::from_value(titles.clone()).expect("titles decode");
    let title = titles.values().next().expect("the title");
    assert_eq!(title.value.name, "Chief Mathematician");
    assert_eq!(
        title.value.kind.as_deref(),
        Some("role"),
        "a value the card recorded as a role must not become a job title"
    );
}

/// The order in the editor is the order on the card, and the card's first address is the one
/// the avatar and the list row are keyed on. A map sorted by a retained id would put a
/// reordered list back the way it was.
#[test]
fn reordering_the_addresses_reorders_them_on_the_card() {
    let mut changed = ContactEdit::from_card(&stored());
    changed.emails = vec!["ada@second.test".into(), "ada@example.test".into()];
    let patch = build_contact_patch(&stored(), &changed).expect("a valid patch");
    let FieldPatch::Set(emails) = &patch.fields[&ContactField::Emails] else {
        panic!("emails were cleared");
    };
    let emails: BTreeMap<PropertyId, ContactProperty<ContactEmail>> =
        serde_json::from_value(emails.clone()).expect("emails decode");
    assert_eq!(
        emails
            .values()
            .map(|entry| entry.value.address.clone())
            .collect::<Vec<_>>(),
        vec!["ada@second.test".to_owned(), "ada@example.test".to_owned()]
    );
}

#[test]
fn emptying_a_field_clears_it_rather_than_leaving_it() {
    let mut changed = ContactEdit::from_card(&stored());
    changed.organization = String::new();
    changed.phones = Vec::new();
    let patch = build_contact_patch(&stored(), &changed).expect("a valid patch");
    let FieldPatch::Set(organizations) = &patch.fields[&ContactField::Organizations] else {
        panic!("the organisation field was not named");
    };
    assert_eq!(organizations, &serde_json::json!({}));
    let FieldPatch::Set(phones) = &patch.fields[&ContactField::Phones] else {
        panic!("the phone field was not named");
    };
    assert_eq!(phones, &serde_json::json!({}));
}

/// A card with nothing to file it under, and a value that is not an address, are both refused
/// before anything reaches the server. Neither message names a value: an address is content,
/// and these strings reach the diagnostic log.
#[test]
fn an_unfilable_card_and_a_malformed_address_are_refused() {
    let error =
        build_contact_draft(book(), "uid", &ContactEdit::default()).expect_err("an empty contact");
    assert!(matches!(error, AccountError::ContactWrite(_)));
    for malformed in ["ada", "@example.test", "ada@", "ada@@example.test"] {
        let broken = ContactEdit {
            given_name: "Ada".into(),
            emails: vec![malformed.into()],
            ..ContactEdit::default()
        };
        let error = build_contact_draft(book(), "uid", &broken).expect_err(malformed);
        assert!(
            !format!("{error}").contains(malformed),
            "the error quoted the address back: {error}"
        );
    }
    assert!(build_contact_patch(&stored(), &ContactEdit::default()).is_err());
}

/// Whitespace a form leaves behind is the caller's to lose, not the server's to store.
#[test]
fn a_form_is_trimmed_and_its_blank_rows_dropped() {
    let padded = ContactEdit {
        given_name: "  Ada  ".into(),
        surname: " Lovelace ".into(),
        organization: "  ".into(),
        title: String::new(),
        emails: vec!["  ada@example.test ".into(), "   ".into()],
        phones: vec![String::new()],
    };
    let draft = build_contact_draft(book(), "uid", &padded).expect("a valid draft");
    assert_eq!(draft.card.display_name().as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        draft
            .card
            .emails
            .values()
            .map(|entry| entry.value.address.clone())
            .collect::<Vec<_>>(),
        vec!["ada@example.test".to_owned()]
    );
    assert!(draft.card.phones.is_empty());
    assert!(draft.card.organizations.is_empty());
}

/// A card that carries a formatted name and no structured one seeds the whole name into the
/// given-name field, so that an unedited save writes back exactly what was there.
#[test]
fn a_card_without_components_seeds_its_whole_name() {
    let mut card = stored();
    card.name = Some(ContactName {
        full: Some("Ada Lovelace".into()),
        ..ContactName::default()
    });
    let read = ContactEdit::from_card(&card);
    assert_eq!(read.given_name, "Ada Lovelace");
    assert!(read.surname.is_empty());
    assert!(
        build_contact_patch(&card, &read)
            .expect("a valid patch")
            .fields
            .is_empty()
    );
}
