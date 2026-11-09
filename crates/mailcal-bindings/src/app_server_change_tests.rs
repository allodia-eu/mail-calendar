//! Allowing a server found on another site. Every server here is a local port nothing listens on,
//! so allowing one reaches its dial and is refused there; what these pin is that a refused change
//! leaves the account paused on the servers it had, and that the question says so.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use super::ServerChangeAnswer;
use crate::{
    AccountPageIntent, AccountServerChange, ControlState, Intent, LogLevel, MailcalApp,
    MailcalError,
    account_work::Start,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const CALENDAR_ONLY: &str =
    "[caldav]\nbase_url = \"http://127.0.0.1:1/dav\"\nusername = \"alice\"\npassword = \"pw\"\n";

fn app(name: &str) -> (Arc<MailcalApp>, String) {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![CALENDAR_ONLY.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots");
    let id = app.accounts_snapshot().accounts[0].id.clone();
    (app, id)
}

fn pause(app: &MailcalApp, id: &str) {
    pause_on(app, id, "example.net");
}

fn pause_on(app: &MailcalApp, id: &str, site: &str) {
    app.app.note_server_change(
        &engine_api::AccountId::try_from(id).unwrap(),
        mailcal_app::ServerChange {
            previous_site: "example.com".to_owned(),
            proposed_site: site.to_owned(),
            calendar_url: Some("http://127.0.0.1:1/elsewhere".to_owned()),
            contacts_url: None,
        },
    );
}

/// The question [`pause`] puts to the person.
fn shown() -> crate::ServerChange {
    crate::ServerChange {
        previous_site: "example.com".to_owned(),
        proposed_site: "example.net".to_owned(),
        calendar: true,
        contacts: false,
    }
}

fn allow(app: &MailcalApp, id: &str, shown: crate::ServerChange) {
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::AllowServerChange {
            account: id.to_owned(),
            shown,
        },
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.account_work.of(id).0.busy {
        assert!(Instant::now() < deadline, "the allow never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The account as Settings lists it, which the question names it by.
fn listed(app: &MailcalApp) -> crate::LinkedAccount {
    let entry = app.accounts_snapshot().accounts.remove(0);
    crate::LinkedAccount {
        id: entry.id,
        address: entry.address,
        distinction: entry.distinction,
    }
}

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

#[test]
fn the_question_reads_the_change_the_account_waits_on() {
    let (app, id) = app("server-change-read");
    assert_eq!(app.account_server_change(id.clone()), None);

    pause(&app, &id);

    assert_eq!(
        app.account_server_change(id),
        Some(AccountServerChange {
            account: listed(&app),
            change: shown(),
            allow: FREE,
            failed: false,
        })
    );
}

/// While an allow runs, the question shows the change it allows, running, and every page holds.
#[test]
fn an_allow_under_way_draws_the_change_it_allows_running() {
    let (app, id) = app("server-change-running");
    pause(&app, &id);
    let intent = AccountPageIntent::AllowServerChange {
        account: id.clone(),
        shown: shown(),
    };
    assert!(matches!(
        app.account_work.start(intent.clone()),
        Start::Run(_)
    ));
    // A lookup replacing the question meanwhile does not move what is being allowed.
    pause_on(&app, &id, "example.org");

    let question = app.account_server_change(id.clone()).expect("open");
    assert_eq!(question.change, shown());
    assert_eq!(
        question.allow,
        ControlState {
            enabled: false,
            running: true,
        }
    );
    assert!(
        matches!(app.account_work.start(intent), Start::Dropped),
        "one press"
    );

    app.account_work.finish(&id, None);
    assert_eq!(
        app.account_server_change(id)
            .map(|question| question.change.proposed_site),
        Some("example.org".to_owned())
    );
}

/// An allow for a question already settled, from another surface or by the use connecting again,
/// is not a failure: nothing waits, so the question closes.
#[test]
fn allowing_a_question_already_settled_says_it_is_settled() {
    let (app, id) = app("server-change-none");
    assert_eq!(
        app.accept_server_change(&id, &shown()).ok(),
        Some(ServerChangeAnswer::Settled)
    );
}

#[test]
fn a_new_server_that_does_not_connect_leaves_the_account_paused_where_it_was() {
    let (app, id) = app("server-change-refused");
    pause(&app, &id);
    let before = app.registry.endpoints(&id);

    allow(&app, &id, shown());

    assert_eq!(
        app.account_server_change(id.clone()),
        Some(AccountServerChange {
            account: listed(&app),
            change: shown(),
            allow: FREE,
            failed: true,
        }),
        "still asked about, saying the allow failed"
    );
    assert_eq!(app.registry.endpoints(&id), before);
    assert_eq!(app.account_page_notice(id), None, "said by the question");
}

/// A failed allow is said only while its change is still the one waiting: once a lookup replaced
/// it, the same servers proposed again later are a new question.
#[test]
fn a_failure_is_forgotten_once_its_question_is_over() {
    let (app, id) = app("server-change-forgotten");
    pause(&app, &id);
    allow(&app, &id, shown());
    assert!(app.account_server_change(id.clone()).expect("open").failed);

    pause_on(&app, &id, "example.org");
    let _ = app.notices();
    pause(&app, &id);

    assert!(!app.account_server_change(id).expect("open").failed);
}

/// A lookup can replace the question while it is on screen. Allowing what was shown must not
/// move the account to a site it never named: nothing moves, and the question asks the new one.
#[test]
fn allowing_a_question_a_lookup_replaced_moves_nothing_and_asks_the_new_one() {
    let (app, id) = app("server-change-replaced");
    pause(&app, &id);
    pause_on(&app, &id, "example.org");
    let before = app.registry.endpoints(&id);

    allow(&app, &id, shown());

    let now = crate::ServerChange {
        proposed_site: "example.org".to_owned(),
        ..shown()
    };
    assert_eq!(
        app.account_server_change(id.clone()),
        Some(AccountServerChange {
            account: listed(&app),
            change: now,
            allow: FREE,
            failed: false,
        })
    );
    assert_eq!(app.registry.endpoints(&id), before);
}

/// A mailbox that connects does not carry an allow on its own: a use that moves and did not
/// connect on its new server refuses it, naming that use.
#[test]
fn a_moved_use_that_did_not_connect_refuses_the_allow_beside_a_working_mailbox() {
    use mailcal_account::Capability;
    use mailcal_app::{UseProblem, UseProblems};

    use crate::account_repair::unconnected;

    let calendar_down = UseProblems {
        calendar: Some(UseProblem::Unreachable),
        contacts: None,
    };
    assert!(matches!(
        unconnected(calendar_down, &[Capability::Calendar]),
        Some(MailcalError::UseNotConnected {
            capability: crate::AccountCapability::Calendar,
            problem: crate::UseProblem::Unreachable,
        })
    ));
    assert!(
        unconnected(calendar_down, &[Capability::Contacts]).is_none(),
        "only the uses that move have to connect"
    );
    assert!(unconnected(calendar_down, &[]).is_none());
}
