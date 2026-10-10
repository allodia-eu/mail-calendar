//! The accounts snapshot over a booted app, and what removal does to the links it shows. Every
//! server is a local port nothing listens on, so the accounts stay listed and never connect.

use std::sync::{Arc, mpsc};

use crate::{
    AccountCapability, AccountKind, CapabilityState, LinkSlot, LogLevel, MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const MAILBOX: &str = r#"
[links]
calendar = "alice@dav:127.0.0.1:1"

[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "alice@example.org"
password = "pw"
"#;

const CLOUD: &str = r#"
[caldav]
base_url = "http://127.0.0.1:1/dav"
username = "alice"
password = "pw"
"#;

const MAILBOX_ID: &str = "alice@example.org@localhost";
const CLOUD_ID: &str = "alice@dav:127.0.0.1:1";

fn app(name: &str, store: &Arc<RecordingCredentialStore>) -> Arc<MailcalApp> {
    app_with(name, store, &[MAILBOX, CLOUD])
}

fn app_with(
    name: &str,
    store: &Arc<RecordingCredentialStore>,
    configs: &[&str],
) -> Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs.iter().map(|config| (*config).to_owned()).collect(),
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(store))),
    )
    .expect("the app boots")
}

#[test]
fn settings_lists_every_account_and_the_links_between_them() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-snapshot", &store);
    let accounts = app.accounts_snapshot().accounts;

    let ids: Vec<&str> = accounts.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(
        ids,
        [MAILBOX_ID, CLOUD_ID],
        "in the order the host stored them"
    );
    assert_eq!(accounts[0].address, "alice@example.org");
    assert_eq!(accounts[0].kind, AccountKind::Imap);
    assert_eq!(accounts[1].kind, AccountKind::Dav);
    let mail = accounts[1]
        .uses
        .iter()
        .find(|row| row.capability == AccountCapability::Mail)
        .expect("listed");
    assert_eq!(mail.state, CapabilityState::Off);

    let calendar = accounts[0].links.calendar.as_ref().expect("linked");
    assert_eq!(calendar.address, "alice");
    assert_eq!(accounts[1].linked_from.len(), 1);
    assert_eq!(accounts[1].linked_from[0].id, MAILBOX_ID);
}

#[test]
fn removing_an_account_clears_the_links_to_it_and_stores_the_change() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-remove-link", &store);

    app.remove_account(CLOUD_ID.to_owned()).expect("removed");

    let accounts = app.accounts_snapshot().accounts;
    assert_eq!(accounts.len(), 1);
    assert!(accounts[0].links.calendar.is_none());
    let stored = store
        .persisted
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(id, _)| id == MAILBOX_ID)
        .map(|(_, config)| config.clone())
        .expect("the mailbox's config was stored again");
    assert!(!stored.contains("links"), "{stored}");
    assert!(stored.contains("alice@example.org"));

    app.remove_account(MAILBOX_ID.to_owned()).expect("removed");
    assert!(
        app.accounts_snapshot().accounts.is_empty(),
        "no account left, so the client returns to first-run setup"
    );
}

/// A rotation that lands between serializing an entry and storing it leaves the store holding
/// what the registry holds, not the serialization taken before it.
#[test]
fn a_config_superseded_while_it_was_stored_is_stored_again() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-superseded", &store);
    app.persist_config(MAILBOX_ID, "superseded = true".to_owned())
        .expect("stored");
    let writes: Vec<String> = store
        .persisted
        .lock()
        .unwrap()
        .iter()
        .filter(|(id, _)| id == MAILBOX_ID)
        .map(|(_, config)| config.clone())
        .collect();
    assert_eq!(writes.len(), 2);
    assert!(writes[1].contains("alice@example.org"), "{}", writes[1]);
}

#[test]
fn naming_a_calendar_s_mail_account_links_both_ends_and_stores_them() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-set-link", &store);
    app.set_account_link(MAILBOX_ID.to_owned(), LinkSlot::Calendar, None)
        .expect("cleared");
    assert!(app.accounts_snapshot().accounts[0].links.calendar.is_none());

    app.set_account_link(
        CLOUD_ID.to_owned(),
        LinkSlot::Mail,
        Some(MAILBOX_ID.to_owned()),
    )
    .expect("linked");

    let accounts = app.accounts_snapshot().accounts;
    assert_eq!(
        accounts[0]
            .links
            .calendar
            .as_ref()
            .map(|linked| linked.id.as_str()),
        Some(CLOUD_ID)
    );
    assert_eq!(
        accounts[1]
            .links
            .mail
            .as_ref()
            .map(|linked| linked.id.as_str()),
        Some(MAILBOX_ID)
    );
    let last_stored = |account: &str| {
        store
            .persisted
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(id, _)| id == account)
            .map(|(_, config)| config.clone())
            .unwrap_or_default()
    };
    assert!(last_stored(MAILBOX_ID).contains(&format!("calendar = \"{CLOUD_ID}\"")));
    assert!(last_stored(CLOUD_ID).contains(&format!("mail = \"{MAILBOX_ID}\"")));
}

#[test]
fn a_link_the_slot_may_not_name_is_refused_and_changes_nothing() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-refuse-link", &store);
    let refused = app.set_account_link(
        MAILBOX_ID.to_owned(),
        LinkSlot::Mail,
        Some(CLOUD_ID.to_owned()),
    );
    assert!(matches!(refused, Err(crate::MailcalError::Config(_))));
    assert!(app.accounts_snapshot().accounts[0].links.mail.is_none());
    assert!(store.persisted.lock().unwrap().is_empty());
}

fn mail_link(app: &MailcalApp, id: &str) -> Option<String> {
    app.accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.id == id)
        .and_then(|entry| entry.links.mail)
        .map(|linked| linked.id)
}

#[test]
fn clearing_the_mail_account_a_calendar_sends_through_unlinks_them() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app("accounts-clear-sender", &store);
    assert_eq!(mail_link(&app, CLOUD_ID).as_deref(), Some(MAILBOX_ID));

    app.set_account_link(CLOUD_ID.to_owned(), LinkSlot::Mail, None)
        .expect("cleared");

    assert_eq!(mail_link(&app, CLOUD_ID), None);
    assert!(app.accounts_snapshot().accounts[0].links.calendar.is_none());
}

const SECOND_MAILBOX: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "localhost"
username = "bob@example.org"
password = "pw"
"#;

#[test]
fn a_second_mail_account_linking_a_calendar_keeps_the_one_it_sends_through() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app_with(
        "accounts-second-sender",
        &store,
        &[MAILBOX, CLOUD, SECOND_MAILBOX],
    );

    app.set_account_link(
        "bob@example.org@localhost".to_owned(),
        LinkSlot::Calendar,
        Some(CLOUD_ID.to_owned()),
    )
    .expect("linked");

    assert_eq!(mail_link(&app, CLOUD_ID).as_deref(), Some(MAILBOX_ID));
    let cloud = app
        .accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.id == CLOUD_ID)
        .unwrap();
    assert_eq!(cloud.linked_from.len(), 2);
}

/// What a client asks to decide on first-run setup: a device holding only a calendar-and-contacts
/// account has accounts, though none is in the mailbox snapshot; and removing it leaves none.
#[test]
fn an_account_without_mail_is_an_account_until_it_is_removed() {
    let store = Arc::new(RecordingCredentialStore::default());
    let app = app_with("accounts-without-mail", &store, &[CLOUD]);
    assert!(app.mailbox_list().accounts.is_empty(), "no mail surface");
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);

    app.remove_account(CLOUD_ID.to_owned()).expect("removed");
    assert!(
        app.accounts_snapshot().accounts.is_empty(),
        "the last account is gone"
    );
}
