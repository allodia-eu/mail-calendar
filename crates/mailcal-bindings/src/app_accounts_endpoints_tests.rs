//! Editing an account's servers. Every server is a local port nothing listens on, so an edit that
//! reaches its dial is refused there; what these pin is that a refused edit changes nothing.

use std::sync::{Arc, mpsc};

use crate::{
    LogLevel, MailcalApp, MailcalError,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const ALICE: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"

[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice@example.org"
password = "pw"
"#;

const BOB: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "bob@example.org"
password = "pw"
"#;

const ALICE_ID: &str = "alice@example.org@localhost";

fn app(name: &str, store: &Arc<RecordingCredentialStore>) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![ALICE.to_owned(), BOB.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(store))),
    )
    .expect("the app boots")
}

fn endpoints(app: &MailcalApp) -> crate::AccountEndpoints {
    app.accounts_snapshot().accounts[0]
        .endpoints
        .clone()
        .expect("a password account's servers are editable")
}

#[test]
fn settings_reads_the_servers_back_without_the_password() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("endpoints-read", &store);
    let read = endpoints(&app);
    assert_eq!(read.imap_host.as_deref(), Some("127.0.0.1:1"));
    assert_eq!(read.caldav_url.as_deref(), Some("http://127.0.0.1:1/dav"));
    assert_eq!(read.username, "alice@example.org");
    assert!(read.password.is_none());
}

#[test]
fn servers_that_do_not_connect_leave_the_account_as_it_was() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("endpoints-unreachable", &store);
    let mut edit = endpoints(&app);
    edit.imap_host = Some("127.0.0.1:2".to_owned());
    edit.password = Some("new".to_owned());

    let refused = app.update_account_endpoints(ALICE_ID.to_owned(), edit);

    assert!(refused.is_err(), "nothing listens there");
    assert_eq!(
        endpoints(&app).imap_host.as_deref(),
        Some("127.0.0.1:1"),
        "the old servers are still the account's"
    );
    assert!(store.persisted.lock().unwrap().is_empty(), "nothing stored");
}

#[test]
fn an_edit_that_would_duplicate_an_account_or_strand_a_use_is_refused() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("endpoints-refused", &store);

    let mut bob = endpoints(&app);
    bob.username = "bob@example.org".to_owned();
    assert!(matches!(
        app.update_account_endpoints(ALICE_ID.to_owned(), bob),
        Err(MailcalError::Config(_))
    ));

    let mut no_calendar = endpoints(&app);
    no_calendar.caldav_url = None;
    assert!(matches!(
        app.update_account_endpoints(ALICE_ID.to_owned(), no_calendar),
        Err(MailcalError::Config(_))
    ));
    assert!(store.persisted.lock().unwrap().is_empty());
}
