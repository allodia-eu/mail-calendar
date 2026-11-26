//! An account whose own server is reached only over plaintext: neither an outage nor a refused
//! sign-in, and gone once the account connects or is removed.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app};

use crate::{protocol::Surface, tests_fakes as fakes};

#[test]
fn an_insecure_server_takes_the_outage_badge_away_and_signals_only_a_change() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_account_unreachable(&id, Some("a: connect failed".to_owned()));

    app.note_insecure_server(&id, mailcal_account::InsecureCause::PlaintextAddress);

    assert_eq!(
        app.insecure_server_accounts(),
        [(
            "a".to_owned(),
            mailcal_account::InsecureCause::PlaintextAddress
        )]
    );
    assert_eq!(
        app.connectivity().unreachable_accounts,
        Vec::<String>::new(),
        "no outage: waiting changes nothing"
    );
    assert_eq!(
        app.connectivity().signin_expired_accounts,
        Vec::<String>::new()
    );
    surfaces.lock().unwrap().clear();
    app.note_insecure_server(&id, mailcal_account::InsecureCause::PlaintextAddress);
    assert_eq!(*surfaces.lock().unwrap(), Vec::new(), "nothing changed");

    app.clear_insecure_server(&id);
    assert_eq!(app.insecure_server_accounts(), Vec::new());
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
}

#[tokio::test]
async fn removing_the_account_takes_it_away() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("a", FakeProvider::new())], &surfaces);
    let id = engine_api::AccountId::try_from("a").unwrap();
    app.note_insecure_server(&id, mailcal_account::InsecureCause::PlaintextAddress);

    app.remove_account(&id).await;

    assert_eq!(app.insecure_server_accounts(), Vec::new());
}

/// A calendar or address book reached only over plaintext is one a retry dials again, whichever
/// the cause, as an unreachable one is: the server or its address may have changed since.
#[test]
fn a_use_reached_only_over_plaintext_is_one_a_retry_dials_again() {
    use mailcal_account::InsecureCause;
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("a", FakeProvider::new()),
            account("b", FakeProvider::new()),
            account("c", FakeProvider::new()),
        ],
        &surfaces,
    );
    let id = |id: &str| engine_api::AccountId::try_from(id).unwrap();
    let problems = |calendar, contacts| crate::UseProblems { calendar, contacts };
    let insecure = |cause| Some(crate::UseProblem::Insecure(cause));
    app.note_use_problems(
        &id("a"),
        problems(insecure(InsecureCause::PlaintextAddress), None),
    );
    app.note_use_problems(
        &id("b"),
        problems(None, insecure(InsecureCause::LedToPlaintext)),
    );
    app.note_use_problems(
        &id("c"),
        problems(Some(crate::UseProblem::NoneOnServer), None),
    );

    let mut again = app.accounts_with_insecure_use();
    again.sort();
    assert_eq!(again, ["a", "b"]);
}
