//! An address book that connected and then stops answering: its refreshes say so, it is offered
//! for no change while they do, and a refresh that reaches it again takes that away.

use engine_api::AccountId;

use super::*;
use crate::{ContactWriteStatus, Intent, UseProblem, UseProblems};

fn create() -> Intent {
    Intent::Contacts(ContactsIntent::CreateContact {
        account: None,
        address_book: None,
        edit: mailcal_account::ContactEdit {
            given_name: "Grace".to_owned(),
            surname: "Hopper".to_owned(),
            emails: vec!["grace@example.test".to_owned()],
            ..mailcal_account::ContactEdit::default()
        },
    })
}

/// Whether the one listed person offers an edit.
async fn editable(app: &crate::App<fake::MailOnly>) -> bool {
    let row = app.contacts().rows.remove(0);
    let detail = app.contact_detail(&row.id).await.expect("the detail");
    !detail.editable_cards.is_empty()
}

async fn refresh(app: &crate::App<fake::MailOnly>) {
    app.dispatch(Intent::Contacts(ContactsIntent::RefreshContacts))
        .await;
}

#[tokio::test]
async fn an_address_book_that_stops_answering_says_so_and_clears_when_a_refresh_reaches_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let log = WriteLog::default();
    let book = FakeContacts::new(
        "book",
        vec![card("c-ada", "book", "Ada Lovelace", "ada@example.test")],
    )
    .writable()
    .recording(&log);
    let outage = book.outage();
    let app = app(vec![account("acct", vec![Box::new(book)])], &surfaces);
    let id = AccountId::try_from("acct").unwrap();
    refresh(&app).await;
    assert_eq!(app.contact_targets().await.len(), 1);
    assert!(editable(&app).await);

    outage.store(true, Ordering::Relaxed);
    refresh(&app).await;
    assert_eq!(
        app.use_problems(&id),
        UseProblems::default(),
        "one refresh that reached nothing is not an outage"
    );
    surfaces.lock().unwrap().clear();
    refresh(&app).await;
    let unreachable = UseProblems {
        calendar: None,
        contacts: Some(UseProblem::Unreachable),
    };
    assert_eq!(app.use_problems(&id), unreachable);
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert_eq!(
        app.contact_targets().await,
        [],
        "no book is offered for a new contact"
    );
    assert!(
        !editable(&app).await,
        "its cards stay listed and offer no edit"
    );

    app.dispatch(create()).await;
    assert_eq!(app.contact_write_status(), ContactWriteStatus::NotSaved);
    assert_eq!(log.entries(), [], "nothing was sent");

    outage.store(false, Ordering::Relaxed);
    refresh(&app).await;
    assert_eq!(app.use_problems(&id), UseProblems::default());
    assert_eq!(app.contact_targets().await.len(), 1);
    assert!(editable(&app).await);
}
