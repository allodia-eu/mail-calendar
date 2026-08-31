//! What the contacts projection must decide, and what it must leave to the core.

use mailcal_bindings::{AccountRow, ContactDetail, ContactRow, ContactValue};

use super::{ContactsModel, ListState, people};

fn row(id: &str, name: &str, section: &str, accounts: u32) -> ContactRow {
    ContactRow {
        avatar: crate::ui::model::blank_avatar(),
        id: id.to_owned(),
        display_name: name.to_owned(),
        primary_email: format!("{id}@example.test"),
        section: section.to_owned(),
        account_count: accounts,
    }
}

fn account(id: &str, email: &str) -> AccountRow {
    AccountRow {
        id: id.to_owned(),
        email: email.to_owned(),
        expanded: true,
    }
}

fn value(value: &str, accounts: &[&str]) -> ContactValue {
    ContactValue {
        value: value.to_owned(),
        accounts: accounts.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn detail(name: &str, accounts: &[&str]) -> ContactDetail {
    ContactDetail {
        avatar: crate::ui::model::blank_avatar(),
        id: "person".to_owned(),
        display_name: name.to_owned(),
        emails: vec![value("eva@work.test", &["acct-1"])],
        phones: Vec::new(),
        organizations: Vec::new(),
        titles: Vec::new(),
        accounts: accounts.iter().map(|id| (*id).to_owned()).collect(),
    }
}

/// The header rides on the row that starts a section, decided by comparing with the row *above*.
/// Re-bucketing by key here would be a second ordering that could disagree with the core's; and
/// the core's is the one every other client renders.
#[test]
fn a_section_letter_is_drawn_only_where_it_changes() {
    let rows = people(&[
        row("a1", "Ada", "A", 1),
        row("a2", "Aisha", "A", 1),
        row("b1", "Bram", "B", 1),
        row("d1", "9 Lives Vet", "#", 1),
    ]);

    let sections = rows
        .iter()
        .map(|row| row.section.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        sections,
        vec![
            Some("A".to_owned()),
            None,
            Some("B".to_owned()),
            Some("#".to_owned()),
        ]
    );
}

/// A merged row must say it is a merge: and an ordinary one must not say "In 1 accounts", which
/// is noise on every contact, and ungrammatical noise.
#[test]
fn only_a_real_merge_carries_the_account_disclosure() {
    let rows = people(&[row("one", "Solo", "S", 1), row("two", "Merged", "M", 2)]);

    assert_eq!(rows[0].accounts, None);
    assert_eq!(rows[1].accounts.as_deref(), Some("In 2 accounts"));
}

/// A card may legitimately carry an address and no name. The core emits an **empty** name for it
/// rather than English text a Dutch reader would be stuck with; supplying the placeholder is the
/// client's job.
#[test]
fn a_nameless_card_takes_the_clients_own_placeholder() {
    let rows = people(&[row("anon", "", "#", 1)]);

    assert_eq!(rows[0].name, "(no name)");
    assert_eq!(rows[0].email, "anon@example.test");
}

/// The two empty states are different sentences on purpose: telling someone who has just searched
/// "No contacts yet" reads as though theirs had vanished.
#[test]
fn the_empty_state_turns_on_whether_a_search_is_narrowing() {
    let mut model = ContactsModel::default();
    assert_eq!(model.state(), ListState::NoContacts);

    model.set_query("zz".to_owned());
    assert_eq!(model.state(), ListState::NoResults);

    model.rows = people(&[row("a1", "Ada", "A", 1)]);
    assert_eq!(model.state(), ListState::Rows);

    // Entering the surface drops the narrowing, so a filtered list can never sit under an empty
    // search box.
    model.entered();
    assert_eq!(model.query(), "");
}

/// Two people opened in quick succession answer in whatever order the store's connection thread
/// hands them back. The later **request** wins, never the earlier answer.
#[test]
fn a_stale_detail_answer_never_lands_on_the_person_opened_since() {
    let mut model = ContactsModel::default();
    let first = model.begin_lookup();
    let second = model.begin_lookup();
    assert_ne!(first, second);

    model.finish_lookup(second, Some(&detail("Eva Jansen", &["acct-1"])), &[]);
    model.finish_lookup(first, Some(&detail("Someone Else", &["acct-1"])), &[]);
    assert_eq!(model.opened().expect("a person is open").name, "Eva Jansen");

    // `None` means the person is genuinely gone: never merely renumbered.
    let third = model.begin_lookup();
    model.finish_lookup(third, None, &[]);
    assert!(model.opened().is_none());
}

/// Provenance exists to disambiguate. With one account there is nothing to disambiguate, so
/// repeating the same address down the screen is suppressed: and "Also in" goes with it, because
/// it is the *explanation* of a disclosure the row did not make.
#[test]
fn provenance_is_named_for_a_merge_and_suppressed_for_everyone_else() {
    let accounts = [
        account("acct-1", "eva@work.test"),
        account("acct-2", "eva@home.test"),
    ];

    let mut model = ContactsModel::default();
    let lookup = model.begin_lookup();
    model.finish_lookup(lookup, Some(&detail("Eva", &["acct-1"])), &accounts);
    let alone = model.opened().expect("a person is open");
    assert!(alone.accounts.is_empty(), "one account explains nothing");
    assert_eq!(alone.groups[0].values[0].accounts, "");

    let mut merged = detail("Eva", &["acct-1", "acct-2"]);
    merged.emails = vec![value("eva@work.test", &["acct-1", "acct-2"])];
    let lookup = model.begin_lookup();
    model.finish_lookup(lookup, Some(&merged), &accounts);
    let person = model.opened().expect("a person is open");
    assert_eq!(person.accounts, vec!["eva@work.test", "eva@home.test"]);
    // The user's own word for the account, never the core's internal id.
    assert_eq!(
        person.groups[0].values[0].accounts,
        "eva@work.test, eva@home.test"
    );
}

/// An id whose account has since been removed falls back to itself: a value with no visible source
/// is worse than an ugly one.
#[test]
fn a_value_from_a_removed_account_still_names_where_it_came_from() {
    let mut model = ContactsModel::default();
    let mut merged = detail("Eva", &["acct-1", "gone"]);
    merged.emails = vec![value("eva@work.test", &["gone"])];
    let lookup = model.begin_lookup();
    model.finish_lookup(lookup, Some(&merged), &[account("acct-1", "eva@work.test")]);

    let person = model.opened().expect("a person is open");
    assert_eq!(person.accounts, vec!["eva@work.test", "gone"]);
    assert_eq!(person.groups[0].values[0].accounts, "gone");
}

/// Only the groups a person actually has; an empty one would draw a heading over nothing.
#[test]
fn a_person_carries_only_the_groups_they_have_values_for() {
    let mut model = ContactsModel::default();
    let mut person = detail("Eva", &["acct-1"]);
    person.organizations = vec![value("Allodia", &["acct-1"])];
    let lookup = model.begin_lookup();
    model.finish_lookup(lookup, Some(&person), &[]);

    let headings = model
        .opened()
        .expect("a person is open")
        .groups
        .iter()
        .map(|group| group.heading)
        .collect::<Vec<_>>();
    assert_eq!(headings, vec!["Email", "Organisation"]);
}
