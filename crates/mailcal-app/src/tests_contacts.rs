//! End-to-end contacts tests over a **real in-memory engine**: fake CardDAV-shaped adapters
//! feed cards in, the engine derives its unified people index, and the app projects the
//! snapshot the host renders.
//!
//! Deliberately not mocked at the projection boundary. The headline claim of this feature;
//! "someone in two of your accounts is one contact, not two", is a collaboration between the
//! engine's join and this crate's projection, and a test that stubbed either half would prove
//! only that the half it kept still compiles. These drive the same path the app does.

#[path = "tests_contacts_fake.rs"]
mod fake;

use std::sync::{Arc, Mutex};

// Re-exported, not merely imported: the child modules below read the same fixtures through
// `use super::*`.
pub(crate) use fake::{ContactWriteRecord, FakeContacts, WriteLog, account, app, card};

use crate::Surface;

#[tokio::test]
async fn one_person_in_two_accounts_becomes_one_contact_row() {
    // The feature's headline claim, proven through the real engine join rather than a stub.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                vec![Box::new(FakeContacts::new(
                    "work-book",
                    vec![card(
                        "c-work",
                        "work-book",
                        "Ada Lovelace",
                        "ada@example.test",
                    )],
                ))],
            ),
            account(
                "home",
                vec![Box::new(FakeContacts::new(
                    "home-book",
                    // The SAME address, filed under a different card id in another account.
                    vec![card(
                        "c-home",
                        "home-book",
                        "Ada Lovelace",
                        "ada@example.test",
                    )],
                ))],
            ),
        ],
        &surfaces,
    );

    app.dispatch(crate::Intent::RefreshContacts).await;

    let rows = app.contacts().rows;
    assert_eq!(rows.len(), 1, "two cards, one address → one person");
    assert_eq!(rows[0].display_name, "Ada Lovelace");
    // And it says so, rather than silently presenting two cards as one.
    assert_eq!(rows[0].account_count, 2);
    assert!(surfaces.lock().unwrap().contains(&Surface::Contacts));
}

#[tokio::test]
async fn two_people_sharing_a_name_but_not_an_address_stay_separate() {
    // The conservative half of the same rule, and the one that would hurt if it were wrong:
    // merging on names would fuse two unrelated colleagues into one contact, hiding one of
    // them. Two "John Smith"s with different addresses must remain two rows.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![
                    card("c1", "work-book", "John Smith", "john@example.test"),
                    card("c2", "work-book", "John Smith", "j.smith@other.test"),
                ],
            ))],
        )],
        &surfaces,
    );

    app.dispatch(crate::Intent::RefreshContacts).await;

    let rows = app.contacts().rows;
    assert_eq!(rows.len(), 2, "same name, different addresses → two people");
    assert!(rows.iter().all(|row| row.account_count == 1));
}

#[tokio::test]
async fn a_failing_source_does_not_cost_the_user_the_sources_that_worked() {
    // A shared or unreachable address book is common; losing the personal one because of it
    // is not acceptable. The failing adapter is listed FIRST so an early return would be
    // caught rather than masked by ordering.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![
                Box::new(FakeContacts::failing("broken-book")),
                Box::new(FakeContacts::new(
                    "work-book",
                    vec![card(
                        "c1",
                        "work-book",
                        "Grace Hopper",
                        "grace@example.test",
                    )],
                )),
            ],
        )],
        &surfaces,
    );

    app.dispatch(crate::Intent::RefreshContacts).await;

    let rows = app.contacts().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].display_name, "Grace Hopper");
}

#[tokio::test]
async fn searching_narrows_the_list_and_clearing_restores_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![
                    card("c1", "work-book", "Ada Lovelace", "ada@example.test"),
                    card("c2", "work-book", "Grace Hopper", "grace@example.test"),
                ],
            ))],
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshContacts).await;
    assert_eq!(app.contacts().rows.len(), 2);

    app.dispatch(crate::Intent::SearchContacts {
        query: "grace".to_owned(),
    })
    .await;
    let narrowed = app.contacts().rows;
    assert_eq!(narrowed.len(), 1);
    assert_eq!(narrowed[0].display_name, "Grace Hopper");

    // Clearing resets the filter in the core, so the next visit is not silently still
    // narrowed by a query the user can no longer see (the rule mail search follows).
    app.dispatch(crate::Intent::SearchContacts {
        query: String::new(),
    })
    .await;
    assert_eq!(app.contacts().rows.len(), 2);
}

#[tokio::test]
async fn a_contact_row_opens_its_detail_by_id() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![card("c1", "work-book", "Ada Lovelace", "ada@example.test")],
            ))],
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshContacts).await;

    let row = app.contacts().rows.remove(0);
    let detail = app.contact_detail(&row.id).await.expect("detail resolves");
    assert_eq!(detail.display_name, "Ada Lovelace");
    assert_eq!(detail.emails[0].value, "ada@example.test");
    assert_eq!(detail.accounts, vec!["work"]);

    // A garbage id is a miss, not a panic: the host may hold a stale row across a reset.
    assert!(app.contact_detail("not-a-number").await.is_none());
    assert!(app.contact_detail("999999").await.is_none());
}

#[tokio::test]
async fn a_blank_autosuggest_query_returns_nothing() {
    // Focusing the To field must not drop a list of everyone the user has ever emailed.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![card("c1", "work-book", "Ada Lovelace", "ada@example.test")],
            ))],
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshContacts).await;

    assert!(app.recipient_suggestions("").await.is_empty());
    assert!(app.recipient_suggestions("   ").await.is_empty());
}

#[tokio::test]
async fn autosuggest_matches_a_synced_contact() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            vec![Box::new(FakeContacts::new(
                "work-book",
                vec![card("c1", "work-book", "Ada Lovelace", "ada@example.test")],
            ))],
        )],
        &surfaces,
    );
    app.dispatch(crate::Intent::RefreshContacts).await;

    let matches = app.recipient_suggestions("ada").await;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].email, "ada@example.test");
    assert_eq!(matches[0].display_name, "Ada Lovelace");
    assert!(matches[0].is_saved, "a synced personal card is saved");
}

/// What these same paths write to the diagnostic log. A child module so it can reuse the
/// fixtures above; its own file to keep both within the 500-line limit.
#[path = "tests_contacts_logging.rs"]
mod logging;

#[path = "tests_contacts_write.rs"]
mod write;
