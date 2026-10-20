//! A calendar and address book dialled again beside a working mailbox go in around the mailbox the
//! account already has.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use fakes::{CalendarFake, calendar_account, calendar_app};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn new_uses_keep_the_mailbox_the_account_already_has() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut account = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    account.providers = vec![CalendarFake::with_events(Vec::new())].into();
    account.calendar_providers.clear();
    let id = account.id.clone();
    let app = calendar_app(vec![account], &surfaces);
    let mailbox = Arc::clone(&app.account_handle(&id).await.unwrap().providers);

    // What a dial of the uses alone hands back: a calendar, and no mail providers.
    let redialled = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    assert!(app.replace_uses(redialled).await);

    let live = app.account_handle(&id).await.unwrap();
    assert!(
        Arc::ptr_eq(&live.providers, &mailbox),
        "the mailbox is the one it had"
    );
    assert_eq!(
        live.calendar_providers.len(),
        1,
        "the calendar is the new one"
    );
}

#[tokio::test]
async fn an_account_removed_during_the_dial_is_not_brought_back() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = calendar_app(Vec::new(), &surfaces);

    let gone = calendar_account("gone", CalendarFake::with_events(Vec::new()));
    assert!(!app.replace_uses(gone).await);
    assert!(
        app.account_handle(&AccountId::try_from("gone").unwrap())
            .await
            .is_none()
    );
}
