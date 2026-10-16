//! Adding an account that is already set up. The account id comes from the mailbox (login and
//! server), not from how it signs in, so a password account and a sign-in account for the same
//! address are one account; adding the second must change nothing about the first.

use std::sync::{Arc, mpsc};

use crate::{
    LogLevel, MailcalApp, MailcalError,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const ALICE_WITH_PASSWORD: &str = r#"
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

const ALICE_SIGNED_IN: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"

[oauth]
client_id = "client-abc"
refresh_token = "rt-value"
authorize_endpoint = "https://auth.example.org/authorize"
token_endpoint = "https://auth.example.org/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "imap", "smtp"]
"#;

const ALICE_ID: &str = "alice@example.org@localhost";

fn app(name: &str, store: &Arc<RecordingCredentialStore>) -> Arc<MailcalApp> {
    app_with(name, store, ALICE_WITH_PASSWORD)
}

fn app_with(name: &str, store: &Arc<RecordingCredentialStore>, stored: &str) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![stored.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(store))),
    )
    .expect("the app boots")
}

#[test]
fn adding_an_account_that_is_already_set_up_is_refused_and_changes_nothing() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("duplicate-add", &store);
    let before = app.accounts_snapshot();

    let refused = app.add_account(ALICE_SIGNED_IN.to_owned());

    match refused {
        Err(MailcalError::AlreadySetUp { account_id }) => assert_eq!(account_id, ALICE_ID),
        Err(other) => panic!("expected the add to be refused as already set up, got {other:?}"),
        Ok(_) => panic!("the second add of the same mailbox was accepted"),
    }
    // Refused before anything was replaced: no credential written over the stored password, and
    // the account still has the servers and the calendar it had.
    assert!(store.persisted.lock().unwrap().is_empty());
    assert!(store.deleted.lock().unwrap().is_empty());
    let after = app.accounts_snapshot();
    assert_eq!(after.accounts.len(), before.accounts.len());
    assert_eq!(
        after.accounts[0].endpoints, before.accounts[0].endpoints,
        "the password account keeps its servers"
    );
}

/// An account edited onto these servers keeps the id it had, so the add is caught by what its
/// servers derive, not by its id.
#[test]
fn adding_the_mailbox_an_edited_account_now_is_is_refused() {
    let moved = format!("id = \"alice@example.org@old.example\"\n{ALICE_WITH_PASSWORD}");
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app_with("duplicate-add-moved", &store, &moved);

    match app.add_account(ALICE_SIGNED_IN.to_owned()) {
        Err(MailcalError::AlreadySetUp { account_id }) => {
            assert_eq!(account_id, "alice@example.org@old.example");
        }
        Err(other) => panic!("expected the add to be refused as already set up, got {other:?}"),
        Ok(_) => panic!("the mailbox an edited account now is was added a second time"),
    }
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);
    assert!(store.persisted.lock().unwrap().is_empty());
}

#[test]
fn the_refusal_names_no_address_in_its_text() {
    // The display reaches logs; the id it carries is for the client to act on.
    let error = MailcalError::AlreadySetUp {
        account_id: ALICE_ID.to_owned(),
    };
    assert!(!error.to_string().contains("alice"), "{error}");
}
