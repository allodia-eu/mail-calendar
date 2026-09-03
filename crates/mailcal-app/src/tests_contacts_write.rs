//! Creating and editing a contact, end to end over the real in-memory engine.
//!
//! The claim these gate is the one a plausible implementation gets wrong in silence: **an
//! edit writes to the card it was opened on**, and a person is several cards. Everything else
//! here (the destinations offered, the fields a patch names, the status a refusal earns) is in
//! service of that, because each of them is a way to end up writing to the wrong one, or to
//! rewrite a card the user never touched.

use engine_core::contact::{ContactField, FieldPatch};
use mailcal_account::ContactEdit;

use super::*;
use crate::{ContactWriteStatus, Intent};

fn edit(given: &str, surname: &str, email: &str) -> ContactEdit {
    ContactEdit {
        given_name: given.to_owned(),
        surname: surname.to_owned(),
        emails: vec![email.to_owned()],
        ..ContactEdit::default()
    }
}

/// A read-only card: the flag a sync stores on the record, which is what decides whether the
/// detail offers to edit it.
fn read_only(id: &str, book: &str, name: &str, email: &str) -> engine_api::ContactCard {
    let mut card = card(id, book, name, email);
    card.is_writable = false;
    card
}

/// A client's "save to…" picker is built from this, so a source that is not writable must not
/// appear in it: offering one produces a save that fails on the server after the user has
/// typed everything in.
#[tokio::test]
async fn only_writable_books_are_offered_as_destinations() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "personal",
                vec![Box::new(
                    FakeContacts::new("personal-book", Vec::new()).writable(),
                )],
            ),
            account(
                "work",
                vec![Box::new(FakeContacts::new("directory", Vec::new()))],
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;

    let targets = app.contact_targets().await;
    assert_eq!(targets.len(), 1, "{targets:?}");
    assert_eq!(targets[0].account, "personal");
    assert_eq!(targets[0].address_book, "personal-book");
    assert_eq!(targets[0].name, "Personal");
}

#[tokio::test]
async fn a_created_contact_reaches_the_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", Vec::new()).writable(),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;
    assert!(app.contacts().rows.is_empty());

    app.dispatch(Intent::CreateContact {
        account: None,
        address_book: None,
        edit: edit("Grace", "Hopper", "grace@example.test"),
    })
    .await;

    assert_eq!(app.contact_write_status(), ContactWriteStatus::Saved);
    let rows = app.contacts().rows;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].display_name, "Grace Hopper");
    assert_eq!(rows[0].primary_email, "grace@example.test");
    assert!(
        surfaces.lock().unwrap().contains(&Surface::ContactsStatus),
        "the host was never told the write settled"
    );
}

/// The load-bearing one. A person merged from two accounts has two cards, and an edit must go
/// to the one the client named: writing the merged values to whichever card came first would
/// file the work address book's details in the personal one.
#[tokio::test]
async fn an_edit_writes_to_the_card_it_names_and_no_other() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let personal_writes = WriteLog::default();
    let work_writes = WriteLog::default();
    let app = app(
        vec![
            account(
                "personal",
                vec![Box::new(
                    FakeContacts::new(
                        "personal-book",
                        vec![card(
                            "c-personal",
                            "personal-book",
                            "Ada Lovelace",
                            "ada@example.test",
                        )],
                    )
                    .writable()
                    .recording(&personal_writes),
                )],
            ),
            account(
                "work",
                vec![Box::new(
                    FakeContacts::new(
                        "work-book",
                        vec![card(
                            "c-work",
                            "work-book",
                            "Ada Lovelace",
                            "ada@example.test",
                        )],
                    )
                    .writable()
                    .recording(&work_writes),
                )],
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;

    let row = app.contacts().rows.remove(0);
    assert_eq!(row.account_count, 2, "the two cards did not merge");
    let detail = app.contact_detail(&row.id).await.expect("the detail");
    assert_eq!(
        detail.editable_cards.len(),
        2,
        "{:?}",
        detail.editable_cards
    );
    let target = detail
        .editable_cards
        .iter()
        .find(|card| card.account == "work")
        .expect("the work card");

    app.dispatch(Intent::UpdateContact {
        person: row.id.clone(),
        account: target.account.clone(),
        card: target.card.clone(),
        edit: ContactEdit {
            given_name: "Ada".into(),
            surname: "King".into(),
            emails: vec!["ada@example.test".into()],
            ..ContactEdit::default()
        },
    })
    .await;

    assert_eq!(app.contact_write_status(), ContactWriteStatus::Saved);
    assert!(
        personal_writes.entries().is_empty(),
        "the personal card was written to: {:?}",
        personal_writes.entries()
    );
    let work_entries = work_writes.entries();
    let [ContactWriteRecord::Patched(id, patch)] = work_entries.as_slice() else {
        panic!("the work card was not patched: {work_entries:?}");
    };
    assert_eq!(id.as_str(), "c-work");
    // Only the name changed, so only the name is sent: every field a patch names is replaced,
    // and a replacement built from the form loses what the form cannot show.
    assert_eq!(
        patch.fields.keys().copied().collect::<Vec<_>>(),
        vec![ContactField::Name]
    );
    let FieldPatch::Set(name) = &patch.fields[&ContactField::Name] else {
        panic!("the name was cleared");
    };
    assert_eq!(name["full"], "Ada King");
}

/// A card the account may only read is not a place an edit can go, so the detail offers none
/// and the client shows no edit affordance rather than one that fails on press.
#[tokio::test]
async fn a_read_only_card_offers_no_edit() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "directory",
                vec![read_only(
                    "c-directory",
                    "directory",
                    "Ada Lovelace",
                    "ada@example.test",
                )],
            ))],
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;
    let row = app.contacts().rows.remove(0);
    let detail = app.contact_detail(&row.id).await.expect("the detail");
    assert!(detail.editable_cards.is_empty());
}

/// An editor is seeded from the **card**, never from the person: the person is a merge, and
/// seeding from it would offer the other account's values for saving into this one's book.
#[tokio::test]
async fn an_editor_is_seeded_from_the_card_not_from_the_merged_person() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut work_card = card("c-work", "work-book", "Ada Lovelace", "ada@example.test");
    work_card.emails.insert(
        engine_core::contact::PropertyId::new("e2").unwrap(),
        engine_core::contact::ContactProperty::new(engine_core::contact::ContactEmail::new(
            "ada@work.test",
        )),
    );
    let app = app(
        vec![
            account(
                "personal",
                vec![Box::new(
                    FakeContacts::new(
                        "personal-book",
                        vec![card(
                            "c-personal",
                            "personal-book",
                            "Ada Lovelace",
                            "ada@example.test",
                        )],
                    )
                    .writable(),
                )],
            ),
            account(
                "work",
                vec![Box::new(
                    FakeContacts::new("work-book", vec![work_card]).writable(),
                )],
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;
    let row = app.contacts().rows.remove(0);

    let personal = app
        .contact_card(&row.id, "personal", "c-personal")
        .await
        .expect("the personal card");
    assert_eq!(personal.emails, vec!["ada@example.test".to_owned()]);
    let work = app
        .contact_card(&row.id, "work", "c-work")
        .await
        .expect("the work card");
    assert_eq!(
        work.emails,
        vec!["ada@example.test".to_owned(), "ada@work.test".to_owned()]
    );

    // A card that is not this person's is not reachable by naming it.
    assert!(
        app.contact_card(&row.id, "personal", "c-work")
            .await
            .is_none()
    );
}

/// A save that changed nothing sends nothing: an empty patch would still rewrite the card and
/// bump its revision on every other device the user syncs.
#[tokio::test]
async fn saving_an_unchanged_form_sends_no_write() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let writes = WriteLog::default();
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new(
                    "personal-book",
                    vec![card(
                        "c-personal",
                        "personal-book",
                        "Ada Lovelace",
                        "ada@example.test",
                    )],
                )
                .writable()
                .recording(&writes),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;
    let row_id = app.contacts().rows.remove(0).id;
    let unchanged = app
        .contact_card(&row_id, "personal", "c-personal")
        .await
        .expect("the card");

    app.dispatch(Intent::UpdateContact {
        person: row_id.clone(),
        account: "personal".into(),
        card: "c-personal".into(),
        edit: unchanged,
    })
    .await;

    assert!(writes.entries().is_empty(), "{:?}", writes.entries());
    assert_eq!(app.contact_write_status(), ContactWriteStatus::Saved);

    // A second save that settles the way the first one did still tells the host something: the
    // status signals on a *change*, so a run of saves with the same outcome would otherwise be
    // one signal, and an editor waiting for the outcome of the save it just submitted would sit
    // there. Every save therefore announces itself before anything can refuse it.
    surfaces.lock().unwrap().clear();
    let unchanged = app
        .contact_card(&row_id, "personal", "c-personal")
        .await
        .expect("the card");
    app.dispatch(Intent::UpdateContact {
        person: row_id,
        account: "personal".into(),
        card: "c-personal".into(),
        edit: unchanged,
    })
    .await;
    assert_eq!(app.contact_write_status(), ContactWriteStatus::Saved);
    assert!(
        surfaces.lock().unwrap().contains(&Surface::ContactsStatus),
        "the second save signalled nothing"
    );
}

/// A refusal before anything is sent is its own status, because it is a different sentence on
/// screen: retrying the same form would be refused the same way.
#[tokio::test]
async fn an_unfilable_contact_is_refused_before_anything_is_sent() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let writes = WriteLog::default();
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", Vec::new())
                    .writable()
                    .recording(&writes),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;

    app.dispatch(Intent::CreateContact {
        account: None,
        address_book: None,
        edit: ContactEdit::default(),
    })
    .await;
    assert_eq!(app.contact_write_status(), ContactWriteStatus::Invalid);
    assert!(writes.entries().is_empty());

    app.dispatch(Intent::CreateContact {
        account: None,
        address_book: None,
        edit: edit("Grace", "Hopper", "not-an-address"),
    })
    .await;
    assert_eq!(app.contact_write_status(), ContactWriteStatus::Invalid);
    assert!(writes.entries().is_empty());
}

/// A destination the client named and the core cannot find is a refusal, not a redirection: the
/// picker's account may have been removed since it opened, and filing the contact in whichever
/// book came first would put it in a different account's address book and report success.
#[tokio::test]
async fn a_named_destination_that_is_gone_files_nowhere() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let writes = WriteLog::default();
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", Vec::new())
                    .writable()
                    .recording(&writes),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshContacts).await;

    app.dispatch(Intent::CreateContact {
        account: Some("gone".to_owned()),
        address_book: None,
        edit: edit("Grace", "Hopper", "grace@example.test"),
    })
    .await;
    assert!(writes.entries().is_empty(), "{:?}", writes.entries());
    assert!(app.contacts().rows.is_empty());
    assert_eq!(app.contact_write_status(), ContactWriteStatus::Failed);

    // Naming neither half still means "wherever contacts go", which is the whole picker for a
    // user with one account.
    app.dispatch(Intent::CreateContact {
        account: None,
        address_book: None,
        edit: edit("Grace", "Hopper", "grace@example.test"),
    })
    .await;
    assert_eq!(app.contacts().rows.len(), 1);
}
