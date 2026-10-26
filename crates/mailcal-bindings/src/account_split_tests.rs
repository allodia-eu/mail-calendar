//! Moving an account's calendar and contacts to an account of their own, over the real FFI
//! constructor. No server here answers, so what is under test is what is refused and what a
//! failure leaves behind; a move that connects is the live suite's
//! (`tests/live_calendar_split.rs`).

use std::sync::{Arc, mpsc};

use crate::{
    AccountCapability, LogLevel, MailcalApp, MailcalError, UnreachedUses,
    account_registry::SignInConfig,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// A grant from an issuer whose tokens reach only its own host, with no calendar scope.
fn grant() -> mailcal_account::OAuthGrant {
    mailcal_account::OAuthGrant {
        client_id: "client-abc".to_owned(),
        client_secret: None,
        refresh_token: mailcal_account::Secret::new("issued".to_owned()),
        authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
        token_endpoint: "http://127.0.0.1:1/token".to_owned(),
        redirect_uri: "mailcal.test://imap-oauth".to_owned(),
        scopes: vec!["offline_access".to_owned(), "imap".to_owned()],
        resource: None,
        issuer: None,
        authorization_server: None,
        protected_resources: Vec::new(),
    }
}

/// A password account whose calendar and contacts are at a server on another site.
fn with_calendar() -> mailcal_account::AccountConfig {
    mailcal_account::load_str(
        r#"
[imap]
addr = "127.0.0.1:1"
server_name = "127.0.0.1"
username = "alice@example.com"
password = "typed"

[caldav]
base_url = "https://dav.example.invalid"
username = "alice@example.com"
password = "typed"
"#,
    )
    .unwrap()
}

fn booted(name: &str, config: &mailcal_account::AccountConfig) -> (Arc<MailcalApp>, String) {
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config.to_toml().unwrap()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(recorder)),
    )
    .expect("the app boots with placeholders for accounts it cannot reach");
    let id = config.account_id().unwrap().as_str().to_owned();
    (app, id)
}

/// The account as the registry holds it: still a password account with its calendar, and the
/// only account there is.
fn unchanged(app: &MailcalApp, id: &str) {
    let Some(SignInConfig::Imap(config)) = app.registry.sign_in_config(id) else {
        panic!("the account is registered");
    };
    assert!(!config.is_oauth());
    assert!(config.caldav.is_some());
    assert_eq!(
        app.accounts_snapshot().accounts.len(),
        1,
        "no second account"
    );
}

#[test]
fn a_sign_in_that_would_lose_the_calendar_is_not_installed_unasked() {
    let before = with_calendar();
    let (app, id) = booted("split-unasked", &before);
    let signed = before.with_grant(grant());
    assert!(!signed.uses_its_grant_does_not_reach().is_empty());

    let outcome = app.install_signed_in(&id, &before, signed, None);

    assert!(
        matches!(outcome, Err(MailcalError::Config(_))),
        "{outcome:?}"
    );
    unchanged(&app, &id);
}

#[test]
fn a_calendar_server_that_does_not_answer_moves_nothing() {
    let before = with_calendar();
    let (app, id) = booted("split-unanswered", &before);

    let kept = app.install_signed_in(
        &id,
        &before,
        before.with_grant(grant()),
        Some(UnreachedUses::KeepOnPassword),
    );
    assert!(kept.is_err(), "{kept:?}");
    unchanged(&app, &id);

    let linked = app.add_linked_calendar_account(
        id.clone(),
        vec![AccountCapability::Calendar, AccountCapability::Contacts],
        "alice".to_owned(),
        "dav-pw".to_owned(),
    );
    assert!(
        matches!(
            linked,
            Err(MailcalError::UseNotConnected {
                capability: AccountCapability::Calendar,
                problem: crate::UseProblem::Unreachable,
            })
        ),
        "names the use that did not connect, for a client to say in the person's language: \
         {linked:?}"
    );
    unchanged(&app, &id);
}

#[test]
fn stopping_the_calendar_with_a_mailbox_that_does_not_answer_changes_nothing() {
    let before = with_calendar();
    let (app, id) = booted("split-stopped", &before);

    let stopped = app.install_signed_in(
        &id,
        &before,
        before.with_grant(grant()),
        Some(UnreachedUses::StopUsing),
    );
    assert!(stopped.is_err(), "{stopped:?}");
    unchanged(&app, &id);
}

#[test]
fn a_linked_calendar_needs_a_password_and_something_to_move() {
    let before = with_calendar();
    let (app, id) = booted("split-refused", &before);
    let link = |uses: Vec<AccountCapability>, password: &str| {
        app.add_linked_calendar_account(id.clone(), uses, "alice".to_owned(), password.to_owned())
    };

    assert!(matches!(
        link(vec![AccountCapability::Calendar], ""),
        Err(MailcalError::Config(_))
    ));
    assert!(matches!(
        link(vec![AccountCapability::Mail], "dav-pw"),
        Err(MailcalError::Config(_))
    ));
    unchanged(&app, &id);
}
