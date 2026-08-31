//! Tests for [`super`]'s contacts projection.
//!
//! Scoped deliberately. **Whether two cards are the same person is the engine's decision**
//! (it joins on shared canonical email and is tested there), so nothing here re-tests the
//! join. What these cover is what *this* module decides: how a unified person is counted,
//! ordered, and labelled; including the one place a plausible implementation would quietly
//! mislead the user (counting source cards where the UI says "accounts").
//!
//! Split out of `contacts.rs` to keep both files under the 500-line limit.

use std::collections::BTreeSet;

use engine_api::{
    AccountId, CanonicalEmail, ContactId, ContactKind, Person, PersonId, PersonSourceId,
    SourcedValue,
};

use super::*;

/// A source card identity in `account`, with card id `contact`.
fn source(account: &str, contact: &str) -> PersonSourceId {
    PersonSourceId::new(
        AccountId::try_from(account).unwrap(),
        ContactId::try_from(contact).unwrap(),
    )
}

/// A person with `name`, one email, and the given source cards.
///
/// An empty `name` becomes `None`, which is what the engine reports for a card carrying
/// neither a name nor an address; it invents no placeholder, and neither does this.
fn person(id: u64, name: &str, email: &str, sources: &[PersonSourceId]) -> Person {
    let sources: BTreeSet<PersonSourceId> = sources.iter().cloned().collect();
    Person {
        id: PersonId::new(id).unwrap(),
        display_name: (!name.is_empty()).then(|| name.to_owned()),
        sources: sources.clone(),
        kinds: BTreeSet::new(),
        names: Vec::new(),
        emails: if email.is_empty() {
            Vec::new()
        } else {
            vec![SourcedValue {
                value: CanonicalEmail::parse(email).unwrap(),
                sources,
            }]
        },
        phones: Vec::new(),
        organizations: Vec::new(),
        titles: Vec::new(),
        is_saved: true,
        is_writable: false,
    }
}

#[test]
fn a_person_assembled_from_two_accounts_reports_two_accounts() {
    // The engine has already merged these into one person; the row must disclose that it
    // is a merge, which is the product rule the whole feature rests on (docs/contacts.md).
    let merged = person(
        1,
        "Ada Lovelace",
        "ada@example.test",
        &[source("alice", "card-a"), source("bob", "card-b")],
    );
    let snapshot = build(&[merged]);
    assert_eq!(snapshot.rows.len(), 1);
    assert_eq!(snapshot.rows[0].account_count, 2);
}

#[test]
fn two_cards_in_one_account_report_one_account_not_two() {
    // The trap this module exists to avoid. A person filed in *two address books of the
    // same account* has two source cards but ONE account, and the UI says "in N accounts".
    // Counting `sources` (the obvious implementation) would tell the user their single
    // account is two.
    let two_books = person(
        2,
        "Grace Hopper",
        "grace@example.test",
        &[source("alice", "personal-1"), source("alice", "shared-1")],
    );
    let snapshot = build(&[two_books]);
    assert_eq!(snapshot.rows[0].account_count, 1);
}

#[test]
fn rows_are_ordered_case_insensitively_and_ties_break_on_id() {
    let people = vec![
        person(3, "zoe", "zoe@example.test", &[source("alice", "c3")]),
        person(1, "Ada", "ada@example.test", &[source("alice", "c1")]),
        person(2, "Ada", "ada2@example.test", &[source("alice", "c2")]),
        person(4, "Bob", "bob@example.test", &[source("alice", "c4")]),
    ];
    let snapshot = build(&people);
    let names: Vec<&str> = snapshot
        .rows
        .iter()
        .map(|row| row.display_name.as_str())
        .collect();
    // "zoe" sorts last despite its lowercase initial: a byte-order sort would have put it
    // after every capitalized name for the wrong reason, and before them if any name were
    // lowercase. The two "Ada"s keep id order, so the list never reshuffles between builds.
    assert_eq!(names, vec!["Ada", "Ada", "Bob", "zoe"]);
    let ids: Vec<&str> = snapshot
        .rows
        .iter()
        .filter(|row| row.display_name == "Ada")
        .map(|row| row.id.as_str())
        .collect();
    assert_eq!(ids, vec!["1", "2"]);
}

#[test]
fn an_accented_name_sorts_next_to_its_base_letter_not_after_z() {
    // The regression: the sort key was the raw lowercased string, and every accented code
    // point is numerically greater than `z`. So "Émile" landed after "Zoe"; under a second
    // `#` heading past the end of the alphabet, nowhere near the E the user was scrolling to.
    let people = vec![
        person(1, "Zoe Angstrom", "zoe@example.test", &[source("a", "c1")]),
        person(
            2,
            "Émile Durand",
            "emile@example.test",
            &[source("a", "c2")],
        ),
        person(3, "Emma Vos", "emma@example.test", &[source("a", "c3")]),
        person(4, "Ada Lovelace", "ada@example.test", &[source("a", "c4")]),
    ];
    let snapshot = build(&people);
    let names: Vec<&str> = snapshot
        .rows
        .iter()
        .map(|row| row.display_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["Ada Lovelace", "Émile Durand", "Emma Vos", "Zoe Angstrom"]
    );
}

#[test]
fn a_nameless_person_still_renders_with_its_email() {
    // A card may legitimately carry an address and no name. The row must still be openable
    // and still show the address, rather than rendering as a blank line.
    let nameless = person(5, "", "anon@example.test", &[source("alice", "c5")]);
    let row = build(&[nameless]).rows.remove(0);
    // EMPTY, not "(no name)". The core has no locale, so a placeholder here is English on a
    // Dutch device, and a client cannot substitute one it has no way to detect. Empty is the
    // signal the client acts on (`docs/contacts.md` §2).
    assert_eq!(row.display_name, "");
    assert_eq!(row.primary_email, "anon@example.test");
    assert_eq!(row.section, "#");
    // The *monogram* does not follow the name into emptiness: with no name the address
    // supplies the letter, which is the rule `docs/avatars.md` states and the one a mail row
    // has always followed, since its sender line falls back to the address too. The letter
    // still corresponds to text the user can see: the address is right there under the name.
    // A person with neither is what leaves it empty, and only then does a client draw its own
    // person glyph.
    assert_eq!(row.avatar.initials, "A");
}

#[test]
fn sections_and_initials_cover_the_shapes_a_real_address_book_holds() {
    let cases = [
        ("Ada Lovelace", "A", "AL"),
        ("Ada", "A", "A"),
        ("ada lovelace", "A", "AL"),
        ("Ada Byron Lovelace", "A", "AL"),
        // A leading digit or symbol files under `#` rather than minting its own section.
        ("7-Eleven", "#", "7"),
        // An accented letter is a LETTER: it files under its base letter, where a reader looks
        // for it. Filing `Ä` under `#` exiles every Dutch, German and French name to a section
        // past Z. The monogram keeps the real character: only the section folds.
        ("Ärzte Ohne Grenzen", "A", "ÄG"),
        ("Émile Durand", "E", "ÉD"),
        ("Øystein Aas", "O", "ØA"),
        ("Łukasz Nowak", "L", "ŁN"),
        // Outside Latin there is nothing honest to fold to, so `#` it is.
        ("Ωμέγα", "#", "Ω"),
    ];
    for (name, expected_section, expected_initials) in cases {
        let row = build(&[person(9, name, "x@example.test", &[source("a", "c")])])
            .rows
            .remove(0);
        assert_eq!(row.section, expected_section, "section for {name}");
        assert_eq!(
            row.avatar.initials, expected_initials,
            "initials for {name}"
        );
    }
}

#[test]
fn detail_reports_which_accounts_carry_each_value() {
    // The detail view's job: not just "here are the emails", but "this one is in your work
    // account and this one is in both": the explanation behind a merged row.
    let alice = source("alice", "card-a");
    let bob = source("bob", "card-b");
    let both: BTreeSet<PersonSourceId> = [alice.clone(), bob.clone()].into_iter().collect();
    let mut merged = person(1, "Ada Lovelace", "ada@example.test", &[alice, bob.clone()]);
    merged.emails.push(SourcedValue {
        value: CanonicalEmail::parse("ada@work.test").unwrap(),
        sources: [bob].into_iter().collect(),
    });
    merged.organizations.push(SourcedValue {
        value: "Analytical Engines".to_owned(),
        sources: both,
    });

    let detail = detail(&merged);
    assert_eq!(detail.accounts, vec!["alice", "bob"]);
    // The shared address lists both accounts; the work-only one lists just its own.
    assert_eq!(detail.emails[0].value, "ada@example.test");
    assert_eq!(detail.emails[0].accounts, vec!["alice", "bob"]);
    assert_eq!(detail.emails[1].value, "ada@work.test");
    assert_eq!(detail.emails[1].accounts, vec!["bob"]);
    assert_eq!(detail.organizations[0].accounts, vec!["alice", "bob"]);
}

#[test]
fn a_group_card_is_not_a_contact_row() {
    // A vCard `KIND:GROUP` is a container, not a person. It has no address, so it renders as a
    // row whose second line is blank and which opens onto nothing; caught on the harness, whose
    // seeded "Harness Friends" group listed itself among the people.
    let mut group = person(1, "Harness Friends", "", &[source("alice", "group-1")]);
    group.kinds = [ContactKind::Group].into_iter().collect();
    let mut individual = person(
        2,
        "Zoe Angstrom",
        "zoe@example.test",
        &[source("alice", "c1")],
    );
    individual.kinds = [ContactKind::Individual].into_iter().collect();

    let rows = build(&[group, individual]).rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].display_name, "Zoe Angstrom");
}

#[test]
fn an_organization_card_is_still_a_contact() {
    // Only groups are excluded. A company with an address is a contact a user legitimately files,
    // and dropping it would quietly lose data they can see in every other client.
    let mut org = person(
        1,
        "7Kleuren Verf",
        "info@7kleuren.example",
        &[source("alice", "c1")],
    );
    org.kinds = [ContactKind::Organization].into_iter().collect();
    assert_eq!(build(&[org]).rows.len(), 1);
}

#[test]
fn a_person_with_both_an_individual_and_a_group_source_survives() {
    // Defensive: `kinds` is a set over every source card. Excluding on "contains Group" rather
    // than "is only Group" would delete a real person the moment anything group-shaped joined.
    let mut mixed = person(
        1,
        "Ada Lovelace",
        "ada@example.test",
        &[source("alice", "c1")],
    );
    mixed.kinds = [ContactKind::Group, ContactKind::Individual]
        .into_iter()
        .collect();
    assert_eq!(build(&[mixed]).rows.len(), 1);
}
