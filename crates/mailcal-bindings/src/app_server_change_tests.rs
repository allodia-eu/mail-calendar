//! Allowing a server found on another site. Every server here is a local port nothing listens on,
//! so allowing one reaches its dial and is refused there; what these pin is that a refused change
//! leaves the account paused on the servers it had.

use std::sync::{Arc, mpsc};

use crate::{
    LogLevel, MailcalApp, MailcalError,
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
    app.app.note_server_change(
        &engine_api::AccountId::try_from(id).unwrap(),
        mailcal_app::ServerChange {
            previous_site: "example.com".to_owned(),
            proposed_site: "example.net".to_owned(),
            calendar_url: Some("http://127.0.0.1:1/elsewhere".to_owned()),
            contacts_url: None,
        },
    );
}

#[test]
fn the_dialog_reads_the_change_the_account_waits_on() {
    let (app, id) = app("server-change-read");
    assert_eq!(app.pending_server_change(id.clone()), None);

    pause(&app, &id);

    assert_eq!(
        app.pending_server_change(id),
        Some(crate::ServerChange {
            previous_site: "example.com".to_owned(),
            proposed_site: "example.net".to_owned(),
            calendar: true,
            contacts: false,
        })
    );
}

#[test]
fn allowing_a_change_nobody_proposed_is_refused() {
    let (app, id) = app("server-change-none");
    assert!(matches!(
        app.accept_server_change(id),
        Err(MailcalError::Config(_))
    ));
}

#[test]
fn a_new_server_that_does_not_connect_leaves_the_account_paused_where_it_was() {
    let (app, id) = app("server-change-refused");
    pause(&app, &id);
    let before = app.accounts_snapshot().accounts[0].endpoints.clone();

    assert!(app.accept_server_change(id.clone()).is_err());

    assert!(app.pending_server_change(id).is_some(), "still asked about");
    assert_eq!(app.accounts_snapshot().accounts[0].endpoints, before);
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
