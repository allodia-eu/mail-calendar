//! What a dial said about an account's calendar and contacts, as the account's page and the next
//! retry read it.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app};

use super::{UseProblem, UseProblems};
use crate::protocol::Surface;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

const UNREACHABLE_CALENDAR: UseProblems = UseProblems {
    calendar: Some(UseProblem::Unreachable),
    contacts: None,
};

#[test]
fn a_dial_replaces_what_the_one_before_it_said_and_signals_only_a_change() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();

    app.note_use_problems(&id, UNREACHABLE_CALENDAR);
    assert_eq!(app.use_problems(&id), UNREACHABLE_CALENDAR);
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));

    surfaces.lock().unwrap().clear();
    app.note_use_problems(&id, UNREACHABLE_CALENDAR);
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "the same answer again changes nothing on screen"
    );

    let refused_contacts = UseProblems {
        calendar: None,
        contacts: Some(UseProblem::SignInRefused),
    };
    app.note_use_problems(&id, refused_contacts);
    assert_eq!(
        app.use_problems(&id),
        refused_contacts,
        "the calendar that connected this time is no longer a problem"
    );

    app.note_use_problems(&id, UseProblems::default());
    assert!(app.every_use_problem().is_empty());
}

#[test]
fn only_an_unreachable_use_is_one_a_retry_dials_again() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("a", FakeProvider::new()),
            account("b", FakeProvider::new()),
        ],
        &surfaces,
    );
    let a = engine_api::AccountId::try_from("a").unwrap();
    let b = engine_api::AccountId::try_from("b").unwrap();
    app.note_use_problems(&a, UNREACHABLE_CALENDAR);
    app.note_use_problems(
        &b,
        UseProblems {
            calendar: Some(UseProblem::NeedsOwnSignIn),
            contacts: Some(UseProblem::NeedsOwnSignIn),
        },
    );

    let retry: Vec<String> = app
        .accounts_with_use_problem(UseProblem::Unreachable)
        .into_keys()
        .collect();
    assert_eq!(
        retry,
        ["a"],
        "a server that needs its own sign-in is not reached by trying again"
    );
}

#[tokio::test]
async fn removing_an_account_forgets_its_problems() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_use_problems(&id, UNREACHABLE_CALENDAR);

    app.remove_account(&id).await;

    assert_eq!(app.use_problems(&id), UseProblems::default());
}

#[test]
fn a_use_switched_off_keeps_no_problem_and_leaves_the_other_alone() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_use_problems(
        &id,
        UseProblems {
            calendar: Some(UseProblem::NeedsOwnSignIn),
            contacts: Some(UseProblem::NeedsOwnSignIn),
        },
    );

    app.forget_use_problem(&id, true);
    assert_eq!(
        app.use_problems(&id),
        UseProblems {
            calendar: None,
            contacts: Some(UseProblem::NeedsOwnSignIn),
        }
    );
    app.forget_use_problem(&id, false);
    assert!(app.every_use_problem().is_empty());
}
