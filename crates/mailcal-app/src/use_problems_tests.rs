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

fn change(calendar: bool, contacts: bool) -> crate::ServerChange {
    crate::ServerChange {
        previous_site: "example.com".to_owned(),
        proposed_site: "example.net".to_owned(),
        calendar_url: calendar.then(|| "https://dav.example.net/".to_owned()),
        contacts_url: contacts.then(|| "https://books.example.net/".to_owned()),
    }
}

const PAUSED_CALENDAR: UseProblems = UseProblems {
    calendar: Some(UseProblem::ServerChanged),
    contacts: None,
};

#[test]
fn a_server_change_pauses_only_the_use_it_moves() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();

    app.note_server_change(&id, change(true, false));

    assert_eq!(app.use_problems(&id), PAUSED_CALENDAR);
    assert_eq!(app.server_change(&id), Some(change(true, false)));
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert!(
        app.accounts_with_use_problem(UseProblem::Unreachable)
            .is_empty(),
        "a paused use is not dialled again by the next retry"
    );
}

#[test]
fn a_paused_use_stays_paused_while_it_still_does_not_connect() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_server_change(&id, change(true, false));

    app.note_use_problems(&id, UNREACHABLE_CALENDAR);

    assert_eq!(app.use_problems(&id), PAUSED_CALENDAR);
    assert!(app.server_change(&id).is_some());
}

#[test]
fn a_paused_use_that_connects_by_itself_is_no_longer_paused() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_server_change(&id, change(true, false));

    app.note_use_problems(&id, UseProblems::default());

    assert_eq!(app.use_problems(&id), UseProblems::default());
    assert_eq!(app.server_change(&id), None, "nothing is left to ask about");
}

#[test]
fn allowing_the_change_lifts_the_pause() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_server_change(&id, change(true, true));

    app.forget_server_change(&id);

    assert_eq!(app.use_problems(&id), UseProblems::default());
    assert_eq!(app.server_change(&id), None);
}

#[test]
fn the_same_server_change_found_again_changes_nothing_on_screen() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_server_change(&id, change(true, false));
    surfaces.lock().unwrap().clear();

    app.note_server_change(&id, change(true, false));
    assert!(surfaces.lock().unwrap().is_empty());

    app.note_server_change(&id, change(true, true));
    assert!(
        surfaces.lock().unwrap().contains(&Surface::Connectivity),
        "a different change is a different question"
    );
}
