//! What the contacts surface must draw, and what it must never draw.
//!
//! Driven through the real [`ContactsPane::render`], and asserted on the **rendered** labels: an
//! `ActionRow::title()` reads back the string it was handed whatever became of the label, so
//! asserting on the property is a green light for a blank row.
//!
//! Called from the crate's single `gtk::init` test (see [`crate::ui::mailbox::tests`]).

use adw::prelude::*;
use mailcal_bindings::{
    AccountRow, ContactCardRef, ContactDetail, ContactRow, ContactTarget, ContactValue,
};

use super::{super::model::ContactsModel, ContactsPane};
use crate::ui::{
    AppInput,
    mailbox::tests::{glib_records, rendered_labels},
};

fn row(id: &str, name: &str, section: &str, initials: &str, accounts: u32) -> ContactRow {
    let mut avatar = crate::ui::model::blank_avatar();
    avatar.initials = initials.to_owned();
    ContactRow {
        avatar,
        id: id.to_owned(),
        display_name: name.to_owned(),
        primary_email: format!("{id}@example.test"),
        section: section.to_owned(),
        account_count: accounts,
    }
}

fn value(value: &str, accounts: &[&str]) -> ContactValue {
    ContactValue {
        value: value.to_owned(),
        accounts: accounts.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn pane() -> (ContactsPane, relm4::Receiver<AppInput>) {
    let (sender, receiver) = relm4::channel::<AppInput>();
    (
        ContactsPane::new(&adw::ApplicationWindow::builder().build(), sender),
        receiver,
    )
}

fn card(account: &str, card: &str) -> ContactCardRef {
    ContactCardRef {
        account: account.to_owned(),
        card: card.to_owned(),
    }
}

fn shown(pane: &ContactsPane) -> Vec<String> {
    rendered_labels(pane.list.upcast_ref::<gtk::Widget>())
}

fn detail_text(pane: &ContactsPane) -> Vec<String> {
    rendered_labels(pane.detail.upcast_ref::<gtk::Widget>())
}

fn index_of(labels: &[String], text: &str) -> usize {
    labels
        .iter()
        .position(|label| label == text)
        .unwrap_or_else(|| panic!("{text:?} is not on screen: {labels:?}"))
}

/// The A–Z letter is drawn above the row that opens its section, and the merge disclosure only
/// where there is a merge: never an ungrammatical "In 1 accounts" on every ordinary contact.
pub(crate) fn the_list_draws_a_letter_per_section_and_a_badge_only_for_a_merge() {
    let (mut pane, _receiver) = pane();
    let model = ContactsModel::fixture(
        &[
            row("ada", "Ada Lovelace", "A", "AL", 1),
            row("aisha", "Aisha Bakker", "A", "AB", 2),
            row("bram", "Bram de Vries", "B", "BV", 1),
        ],
        "",
        None,
        &[],
    );
    assert_eq!(model.rows()[0].avatar.initials, "AL");
    pane.render(&model);

    let labels = shown(&pane);
    assert!(index_of(&labels, "A") < index_of(&labels, "Ada Lovelace"));
    assert!(index_of(&labels, "Ada Lovelace") < index_of(&labels, "Aisha Bakker"));
    assert!(index_of(&labels, "B") < index_of(&labels, "Bram de Vries"));
    assert_eq!(
        labels.iter().filter(|label| *label == "A").count(),
        1,
        "the second A row shares the first one's header: {labels:?}"
    );
    assert_eq!(
        labels
            .iter()
            .filter(|label| *label == "In 2 accounts")
            .count(),
        1
    );
    assert!(!labels.iter().any(|label| label == "In 1 accounts"));
    assert_eq!(
        pane.list_stack.visible_child_name().as_deref(),
        Some("rows")
    );
}

/// A contact's own text is the server's, and it reaches the screen intact: an ampersand is not an
/// entity, and a markup-shaped organisation is shown, never applied.
///
/// The ampersand sits in **both** halves of a row on purpose. libadwaita re-applies its labels
/// when `use-markup` flips, so a row built in the wrong order still reads correctly: what it
/// leaves behind is a `Failed to set text … from markup` record per row, and only the second half
/// of a row would produce one.
pub(crate) fn a_contacts_own_text_is_never_parsed_as_markup() {
    let (mut pane, _receiver) = pane();
    let merged = ContactDetail {
        avatar: crate::ui::model::blank_avatar(),
        id: "ben".to_owned(),
        display_name: "Ben & Jerry".to_owned(),
        emails: vec![value("<b>wire</b>@example.test", &["work", "home"])],
        phones: Vec::new(),
        organizations: vec![value("Johnson & Johnson", &["work", "home"])],
        titles: Vec::new(),
        accounts: vec!["work".to_owned(), "home".to_owned()],
        editable_cards: Vec::new(),
    };
    let accounts = [
        AccountRow {
            id: "work".to_owned(),
            email: "eva@research-and-development.test".to_owned(),
            expanded: true,
        },
        AccountRow {
            id: "home".to_owned(),
            email: "eva@r&d.test".to_owned(),
            expanded: true,
        },
    ];

    let ((), records) = glib_records(|| {
        pane.render(&ContactsModel::fixture(
            &[row("ben", "Ben & Jerry", "B", "BJ", 2)],
            "",
            Some(&merged),
            &accounts,
        ));
    });

    assert!(
        !records.iter().any(|line| line.contains("from markup")),
        "a contact's name, values and provenance must not be parsed as markup: {records:?}"
    );
    let labels = shown(&pane);
    assert!(
        labels.iter().any(|label| label == "Ben & Jerry"),
        "the name must render as itself, not blank: {labels:?}"
    );
    let detail = detail_text(&pane);
    assert!(
        detail.iter().any(|label| label == "Johnson & Johnson"),
        "an organization's ampersand must survive: {detail:?}"
    );
    assert!(
        detail
            .iter()
            .any(|label| label == "<b>wire</b>@example.test"),
        "a markup-shaped value must be shown verbatim, never styled: {detail:?}"
    );
    assert!(
        detail
            .iter()
            .any(|label| label == "eva@research-and-development.test, eva@r&d.test"),
        "the provenance names both accounts, ampersand and all: {detail:?}"
    );
}

/// "Also in" is the explanation of the list row's disclosure, so it exists exactly where that
/// disclosure does. Read-only is said in as many words rather than left to be inferred.
pub(crate) fn the_detail_names_the_accounts_only_for_a_merge_and_says_it_is_read_only() {
    let (mut pane, _receiver) = pane();
    let accounts = [AccountRow {
        id: "work".to_owned(),
        email: "eva@work.test".to_owned(),
        expanded: true,
    }];
    let alone = ContactDetail {
        avatar: crate::ui::model::blank_avatar(),
        id: "eva".to_owned(),
        display_name: String::new(),
        emails: vec![value("eva@work.test", &["work"])],
        phones: Vec::new(),
        organizations: Vec::new(),
        titles: Vec::new(),
        accounts: vec!["work".to_owned()],
        editable_cards: Vec::new(),
    };

    pane.render(&ContactsModel::fixture(&[], "", Some(&alone), &accounts));
    let detail = detail_text(&pane);
    assert_eq!(
        pane.detail_stack.visible_child_name().as_deref(),
        Some("person")
    );
    assert!(
        detail.iter().any(|label| label == "(no name)"),
        "a nameless card takes the client's placeholder: {detail:?}"
    );
    assert!(
        detail
            .iter()
            .any(|label| label == "This contact can't be edited here."),
        "the read-only note must be on screen: {detail:?}"
    );
    assert!(
        !detail.iter().any(|label| label == "Also in"),
        "one account explains nothing, so there is nothing to explain: {detail:?}"
    );

    let mut merged = alone;
    merged.accounts = vec!["work".to_owned(), "home".to_owned()];
    let accounts = [
        AccountRow {
            id: "work".to_owned(),
            email: "eva@work.test".to_owned(),
            expanded: true,
        },
        AccountRow {
            id: "home".to_owned(),
            email: "eva@home.test".to_owned(),
            expanded: true,
        },
    ];
    pane.render(&ContactsModel::fixture(&[], "", Some(&merged), &accounts));
    let detail = detail_text(&pane);
    assert!(
        detail.iter().any(|label| label == "Also in"),
        "a merged person owes the user the accounts they were assembled from: {detail:?}"
    );
    assert!(
        detail.iter().any(|label| label == "eva@home.test"),
        "{detail:?}"
    );
}

/// The three states of the list column, and the placeholder beside it.
///
/// The two empty headlines are deliberately different sentences, and only the nothing-synced-yet
/// one carries the explanatory line: "they appear here once they have synced" answers a question
/// the searching user did not ask.
pub(crate) fn the_pane_swaps_between_people_an_empty_state_and_the_placeholder() {
    let (mut pane, _receiver) = pane();

    pane.render(&ContactsModel::fixture(&[], "", None, &[]));
    assert_eq!(
        pane.list_stack.visible_child_name().as_deref(),
        Some("empty")
    );
    assert_eq!(pane.empty.title(), "No contacts yet");
    assert_eq!(
        pane.empty.description().as_deref(),
        Some("Contacts from your accounts' address books appear here once they have synced.")
    );
    assert_eq!(
        pane.detail_stack.visible_child_name().as_deref(),
        Some("placeholder")
    );

    pane.render(&ContactsModel::fixture(&[], "zz", None, &[]));
    assert_eq!(pane.empty.title(), "No contacts match your search");
    assert_eq!(
        pane.empty.description().unwrap_or_default(),
        "",
        "a no-results headline needs no explanatory line under it"
    );
    assert_eq!(pane.search.text(), "zz");

    pane.render(&ContactsModel::fixture(
        &[row("ada", "Ada Lovelace", "A", "AL", 1)],
        "",
        None,
        &[],
    ));
    assert_eq!(
        pane.list_stack.visible_child_name().as_deref(),
        Some("rows")
    );
    assert_eq!(pane.search.text(), "", "clearing the query clears the box");
}

/// Activating a person asks the model to open them; by **id**, never by row index: the list holds
/// section headers between the people, so an index would resolve against the wrong row.
pub(crate) fn activating_a_person_opens_them_by_id() {
    let (mut pane, receiver) = pane();
    pane.render(&ContactsModel::fixture(
        &[
            row("ada", "Ada Lovelace", "A", "AL", 1),
            row("bram", "Bram de Vries", "B", "BV", 1),
        ],
        "",
        None,
        &[],
    ));

    let second_person = (0..)
        .map_while(|index| pane.list.row_at_index(index))
        .filter_map(|row| row.downcast::<adw::ActionRow>().ok())
        .nth(1)
        .expect("two people are on screen");
    adw::prelude::ActionRowExt::activate(&second_person);

    match receiver.recv_sync().expect("activating a row dispatches") {
        AppInput::OpenContact(id) => assert_eq!(id, "bram"),
        other => panic!("expected the second person, got {other:?}"),
    }
}

/// Both write affordances are conditional, and each answers a different question: the create
/// button asks whether there is anywhere at all to file a contact, the edit button whether
/// *this* person has a card that can be written. A directory contact answers no to the second
/// and says so in as many words, because a button that fails on press is worse than none.
pub(crate) fn the_write_affordances_appear_only_where_a_write_could_land() {
    let (mut pane, _receiver) = pane();
    let accounts = [AccountRow {
        id: "work".to_owned(),
        email: "eva@work.test".to_owned(),
        expanded: true,
    }];
    let mut person = ContactDetail {
        avatar: crate::ui::model::blank_avatar(),
        id: "eva".to_owned(),
        display_name: "Eva Meijer".to_owned(),
        emails: vec![value("eva@work.test", &["work"])],
        phones: Vec::new(),
        organizations: Vec::new(),
        titles: Vec::new(),
        accounts: vec!["work".to_owned()],
        editable_cards: Vec::new(),
    };

    // Nowhere to save, and nothing to edit.
    pane.render(&ContactsModel::fixture(&[], "", Some(&person), &accounts));
    assert!(
        !pane.create.is_visible(),
        "a create with nowhere to file it"
    );
    assert!(!pane.edit.is_visible(), "an edit with no writable card");
    assert!(
        detail_text(&pane)
            .iter()
            .any(|label| label == "This contact can't be edited here."),
        "a person nothing here can write must say so: {:?}",
        detail_text(&pane)
    );

    // A writable book, and a card this person can be edited through.
    person.editable_cards = vec![card("work", "c-work")];
    pane.render(
        &ContactsModel::fixture(&[], "", Some(&person), &accounts).with_targets(
            &[ContactTarget {
                account: "work".to_owned(),
                address_book: "book".to_owned(),
                name: "Personal".to_owned(),
                is_default: true,
            }],
            &accounts,
        ),
    );
    assert!(pane.create.is_visible());
    assert!(pane.edit.is_visible());
    assert!(
        !detail_text(&pane)
            .iter()
            .any(|label| label == "This contact can't be edited here."),
        "an editable contact must not be told it is not: {:?}",
        detail_text(&pane)
    );
}
