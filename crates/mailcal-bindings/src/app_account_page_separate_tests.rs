//! The separate server's section through `dispatch`: offered under the use that needs it, a
//! Connect that does not reach the server said on the form with the login it was tried with, and
//! Skip switching the use off. No server here answers, so a move that connects is the live
//! suite's (`tests/live_calendar_split.rs`).

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountCapability, AccountChangeRefusal, AccountPage, AccountPageIntent, CapabilityState,
    Intent, LogLevel, MailcalApp, TypedPassword, UseProblem,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
    tests_boot::note_after_boot_dial,
};

const WITH_CALENDAR: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"

[caldav]
base_url = "https://dav.example.invalid/alice/"
username = "alice"
password = "pw"
"#;

const ID: &str = "alice@example.org@localhost";

/// A mailbox whose calendar server wants a sign-in of its own.
fn app(name: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![WITH_CALENDAR.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots");
    note_after_boot_dial(
        &app,
        ID,
        mailcal_app::UseProblems {
            calendar: Some(mailcal_app::UseProblem::NeedsOwnSignIn),
            contacts: None,
        },
    );
    app
}

fn settled(app: &MailcalApp) -> AccountPage {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if !app.account_work.of(ID).0.busy {
            return app
                .account_page(ID.to_owned())
                .expect("the account is listed");
        }
        assert!(Instant::now() < deadline, "the change never ended");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn change(app: &MailcalApp, intent: AccountPageIntent) {
    app.dispatch(Intent::AccountPage { intent });
}

#[test]
fn the_form_is_offered_under_the_use_that_needs_it() {
    let app = app("separate-offered");
    let page = settled(&app);
    let separate = page.separate_server.expect("the form is offered");
    assert_eq!(separate.uses, [AccountCapability::Calendar]);
    assert_eq!(separate.host, "dav.example.invalid");
    assert_eq!(separate.username, "alice");
    assert_eq!(separate.failed, None);
    let under: Vec<_> = page
        .uses
        .iter()
        .filter(|switch| switch.signs_in_separately)
        .map(|switch| switch.capability)
        .collect();
    assert_eq!(under, [AccountCapability::Calendar]);
}

#[test]
fn a_connect_that_does_not_reach_the_server_is_said_on_the_form() {
    let app = app("separate-unreached");
    change(
        &app,
        AccountPageIntent::ConnectSeparateServer {
            account: ID.to_owned(),
            username: "alice-dav".to_owned(),
            password: TypedPassword {
                text: "dav-pw".to_owned(),
            },
        },
    );
    let page = settled(&app);
    let separate = page.separate_server.expect("nothing moved");
    assert_eq!(
        separate.failed,
        Some(AccountChangeRefusal::UseNotConnected {
            capability: AccountCapability::Calendar,
            problem: UseProblem::Unreachable,
        })
    );
    assert_eq!(separate.username, "alice-dav", "drawn with the login tried");
    assert!(separate.connect.enabled && !separate.connect.running);
    assert_eq!(page.notice, None, "the form says it, not the page");
    assert_eq!(
        app.accounts_snapshot().accounts.len(),
        1,
        "no second account"
    );
}

#[test]
fn skip_stops_using_what_needs_the_sign_in() {
    let app = app("separate-skipped");
    change(
        &app,
        AccountPageIntent::StopUsingSeparateServer {
            account: ID.to_owned(),
        },
    );
    let page = settled(&app);
    assert_eq!(page.separate_server, None);
    let calendar = page
        .uses
        .iter()
        .find(|switch| switch.capability == AccountCapability::Calendar)
        .expect("listed");
    assert!(!calendar.on);
    let entry = app.accounts_snapshot().accounts.remove(0);
    assert!(
        entry
            .uses
            .iter()
            .all(|use_| use_.capability != AccountCapability::Calendar
                || use_.state == CapabilityState::Off)
    );
}
