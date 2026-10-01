//! Switching an account's uses on and off. Every server is a local port nothing listens on, so
//! the account stays listed and never connects.

use std::sync::{Arc, mpsc};

use crate::{
    AccountCapability, AccountCredentialStore, CapabilityChange, CapabilityState,
    CredentialStoreError, LogLevel, MailcalApp, MailcalError,
    account_registry::{AccountRegistry, UseChange},
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const WITH_CALENDAR: &str = r#"
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

const MAIL_ONLY: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#;

const ID: &str = "alice@example.org@localhost";

fn app(name: &str, config: &str, store: Box<dyn AccountCredentialStore>) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config.to_owned()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        store,
    )
    .expect("the app boots")
}

fn state(app: &MailcalApp, capability: AccountCapability) -> CapabilityState {
    app.accounts_snapshot().accounts[0]
        .uses
        .iter()
        .find(|row| row.capability == capability)
        .expect("listed")
        .state
}

#[test]
fn switching_a_calendar_off_stores_the_choice_under_a_pinned_id() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app(
        "uses-calendar-off",
        WITH_CALENDAR,
        Box::new(RecordingStoreHandle(Arc::clone(&store))),
    );
    assert_eq!(
        state(&app, AccountCapability::Calendar),
        CapabilityState::On
    );

    let change = app
        .set_account_capability(ID.to_owned(), AccountCapability::Calendar, false)
        .expect("switched off");

    assert_eq!(change, CapabilityChange::Applied);
    assert_eq!(
        state(&app, AccountCapability::Calendar),
        CapabilityState::Off
    );
    assert_eq!(
        state(&app, AccountCapability::Contacts),
        CapabilityState::On
    );
    let (stored_id, stored) = store.persisted.lock().unwrap().last().cloned().unwrap();
    assert_eq!(stored_id, ID);
    assert!(stored.contains(&format!("id = \"{ID}\"")), "{stored}");
    assert!(
        stored.contains(r#"capabilities = ["mail", "contacts"]"#),
        "{stored}"
    );
    assert!(
        stored.contains("[caldav]"),
        "the server is kept for switching it on again"
    );

    let again = app
        .set_account_capability(ID.to_owned(), AccountCapability::Calendar, true)
        .expect("switched on");
    assert_eq!(again, CapabilityChange::Applied);
    assert_eq!(
        state(&app, AccountCapability::Calendar),
        CapabilityState::On
    );
}

#[test]
fn a_use_with_no_server_asks_for_one_and_the_last_use_cannot_go() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app(
        "uses-no-server",
        MAIL_ONLY,
        Box::new(RecordingStoreHandle(Arc::clone(&store))),
    );

    let change = app
        .set_account_capability(ID.to_owned(), AccountCapability::Calendar, true)
        .expect("answered");
    assert_eq!(change, CapabilityChange::NeedsEndpoint);
    assert!(matches!(
        app.set_account_capability(ID.to_owned(), AccountCapability::Mail, false),
        Err(MailcalError::Config(_))
    ));
    assert!(matches!(
        app.set_account_capability(ID.to_owned(), AccountCapability::Colleagues, true),
        Err(MailcalError::Config(_))
    ));
    assert!(
        store.persisted.lock().unwrap().is_empty(),
        "nothing changed"
    );
}

struct RefusingStore;

impl AccountCredentialStore for RefusingStore {
    fn persist(&self, _: String, _: String) -> Result<(), CredentialStoreError> {
        Err(CredentialStoreError::Store("locked".to_owned()))
    }

    fn delete(&self, _: String) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

#[test]
fn a_change_the_store_refuses_is_not_made() {
    let app = app("uses-refused", WITH_CALENDAR, Box::new(RefusingStore));
    assert!(matches!(
        app.set_account_capability(ID.to_owned(), AccountCapability::Calendar, false),
        Err(MailcalError::Connect(_))
    ));
    assert_eq!(
        state(&app, AccountCapability::Calendar),
        CapabilityState::On
    );
}

#[test]
fn a_use_the_grant_does_not_hold_needs_consent_and_one_it_holds_does_not() {
    let config = mailcal_account::MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: Vec::new(),
        refresh_token: mailcal_account::Secret::new("refresh".to_owned()),
        granted_scopes: Some(vec![
            "Mail.ReadWrite".to_owned(),
            "Contacts.ReadWrite".to_owned(),
        ]),
        shape: mailcal_account::AccountShape::read("capabilities = [\"mail\"]").unwrap(),
    };
    let id = config.account_id().unwrap();
    let tokens = mailcal_account::GraphTokenSource::new(
        &config,
        id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .unwrap();
    let registry = AccountRegistry::new();
    registry
        .pre_register(
            id.as_str().to_owned(),
            crate::ConnectedAccount::Microsoft { config, tokens },
        )
        .commit();

    let calendar = registry.set_use(id.as_str(), mailcal_account::Capability::Calendar, true);
    assert!(matches!(calendar, Ok(UseChange::NeedsConsent)));
    let contacts = registry.set_use(id.as_str(), mailcal_account::Capability::Contacts, true);
    assert!(matches!(contacts, Ok(UseChange::Changed { .. })));
}
