//! What a person is told after a contact save that did not succeed: never that it was saved
//! when nothing was sent, and never that nothing was saved when it may have been.

use engine_provider::ProviderError;
use mailcal_account::ContactEdit;

use super::*;
use crate::{ContactWriteStatus, ContactsIntent, Intent};

fn ada() -> engine_api::ContactCard {
    card(
        "c-personal",
        "personal-book",
        "Ada Lovelace",
        "ada@example.test",
    )
}

fn renamed() -> ContactEdit {
    ContactEdit {
        given_name: "Augusta".to_owned(),
        surname: "King".to_owned(),
        emails: vec!["ada@example.test".to_owned()],
        ..ContactEdit::default()
    }
}

async fn edit_ada(app: &crate::App<fake::MailOnly>) -> ContactWriteStatus {
    let person = app.contacts().rows.remove(0).id;
    app.dispatch(Intent::Contacts(ContactsIntent::UpdateContact {
        person,
        account: "personal".into(),
        card: "c-personal".into(),
        edit: renamed(),
    }))
    .await;
    app.contact_write_status()
}

/// A card synced while its address book connected still says its server lets it be edited.
/// Once the book stops connecting, nothing can carry an edit there, so none is offered, and a
/// save that arrives anyway says nothing was saved rather than that it may have been.
#[tokio::test]
async fn a_card_whose_address_book_no_longer_connects_offers_no_edit() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let writes = WriteLog::default();
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", vec![ada()])
                    .writable()
                    .recording(&writes),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    let row = app.contacts().rows.remove(0);
    let connected = app.contact_detail(&row.id).await.expect("the detail");
    assert_eq!(connected.editable_cards.len(), 1);

    app.add_account_deferred(account("personal", Vec::new()))
        .await;

    let cached = app
        .contact_detail(&row.id)
        .await
        .expect("the cached detail");
    assert!(
        cached.editable_cards.is_empty(),
        "{:?}",
        cached.editable_cards
    );
    assert_eq!(edit_ada(&app).await, ContactWriteStatus::NotSaved);
    assert!(writes.entries().is_empty(), "{:?}", writes.entries());
}

/// A server that answered and refused the change kept nothing.
#[tokio::test]
async fn a_save_the_server_refused_saved_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", vec![ada()])
                    .writable()
                    .failing_writes(|| ProviderError::permanent("403 Forbidden")),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    assert_eq!(edit_ada(&app).await, ContactWriteStatus::NotSaved);
}

/// A save whose answer was lost may have landed, so it is not reported as lost.
#[tokio::test]
async fn a_save_whose_answer_was_lost_is_unconfirmed() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "personal",
            vec![Box::new(
                FakeContacts::new("personal-book", vec![ada()])
                    .writable()
                    .failing_writes(|| {
                        ProviderError::needs_confirmation("the connection closed after the request")
                    }),
            )],
        )],
        &surfaces,
    );
    app.dispatch(Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
    assert_eq!(edit_ada(&app).await, ContactWriteStatus::Unconfirmed);
}
