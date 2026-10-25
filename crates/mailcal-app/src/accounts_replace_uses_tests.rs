//! A calendar and address book dialled again beside a working mailbox go in around the mailbox the
//! account already has.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use fakes::{CalendarFake, calendar_account, calendar_app};

use crate::{UseProblem, UseProblems, UsesReplaced};

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
    assert_eq!(
        app.replace_uses(redialled, UseProblems::default()).await,
        UsesReplaced::Replaced
    );

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
    assert_eq!(
        app.replace_uses(gone, UseProblems::default()).await,
        UsesReplaced::Gone
    );
    assert!(
        app.account_handle(&AccountId::try_from("gone").unwrap())
            .await
            .is_none()
    );
}

const CALENDAR_UNREACHED: UseProblems = UseProblems {
    calendar: Some(UseProblem::Unreachable),
    contacts: None,
};

#[tokio::test]
async fn a_redial_that_cannot_reach_a_working_calendar_keeps_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let account = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    let id = account.id.clone();
    let app = calendar_app(vec![account], &surfaces);

    let mut redialled = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    redialled.calendar_providers.clear();
    assert_eq!(
        app.replace_uses(redialled, CALENDAR_UNREACHED).await,
        UsesReplaced::Kept
    );

    let live = app.account_handle(&id).await.unwrap();
    assert_eq!(
        live.calendar_providers.len(),
        1,
        "an outage is not traded for the connection that works"
    );
}

#[tokio::test]
async fn a_retry_of_a_calendar_that_never_connected_installs_what_it_found() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let mut account = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    account.calendar_providers.clear();
    let app = calendar_app(vec![account], &surfaces);

    let mut redialled = calendar_account("acct", CalendarFake::with_events(Vec::new()));
    redialled.calendar_providers.clear();
    assert_eq!(
        app.replace_uses(redialled, CALENDAR_UNREACHED).await,
        UsesReplaced::Replaced,
        "nothing works that an outage could take away"
    );
}
