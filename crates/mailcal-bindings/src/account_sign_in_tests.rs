//! Switching an account's sign-in, over the real FFI constructor. The servers here do not answer,
//! so what is under test is what a switch refuses and what a failure leaves behind; a switch
//! that connects is the live suite's (`tests/live_sign_in_switch.rs`).

use std::sync::{Arc, mpsc};

use super::named_as;
use crate::{
    LogLevel, MailcalApp, MailcalError, SignInMethod,
    account_registry::SignInConfig,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// The grant an account signed in through its provider stores, at an issuer that does not
/// answer.
fn grant() -> mailcal_account::OAuthGrant {
    mailcal_account::OAuthGrant {
        client_id: "client-abc".to_owned(),
        client_secret: None,
        refresh_token: mailcal_account::Secret::new("stored-refresh".to_owned()),
        authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
        token_endpoint: "http://127.0.0.1:1/token".to_owned(),
        redirect_uri: "mailcal.test://imap-oauth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        resource: None,
        issuer: None,
        authorization_server: None,
        protected_resources: Vec::new(),
    }
}

/// An IMAP account for `who@example.com` at a server that does not answer, signed in with a
/// password or through its provider.
fn imap_account(who: &str, signed_in: bool) -> (String, String) {
    let config = mailcal_account::AccountConfig {
        imap: Some(mailcal_account::ImapAccount {
            addr: "127.0.0.1:1".to_owned(),
            server_name: "127.0.0.1".to_owned(),
            username: format!("{who}@example.com"),
            password: (!signed_in).then(|| mailcal_account::Secret::new("typed".to_owned())),
            security: mailcal_account::ConnectionSecurity::ImplicitTls,
        }),
        smtp: None,
        caldav: None,
        carddav: None,
        oauth: signed_in.then(grant),
        certificate_exceptions: Vec::new(),
        shape: mailcal_account::AccountShape::default(),
        dav_starts: mailcal_account::DavStarts::default(),
    };
    let id = config.account_id().expect("a valid account id");
    (
        config.to_toml().expect("serializable config"),
        id.as_str().to_owned(),
    )
}

fn booted(name: &str, configs: Vec<String>) -> (Arc<MailcalApp>, Arc<RecordingCredentialStore>) {
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("the app boots with placeholders for accounts it cannot reach");
    (app, recorder)
}

fn signs_in_through_its_provider(app: &MailcalApp, id: &str) -> bool {
    match app.registry.sign_in_config(id) {
        Some(SignInConfig::Imap(config)) => config.is_oauth(),
        _ => panic!("the account is registered"),
    }
}

#[test]
fn the_snapshot_says_how_each_account_signs_in() {
    let (typed, typed_id) = imap_account("typed", false);
    let (signed_in, signed_in_id) = imap_account("signed-in", true);
    let (app, _) = booted("sign-in-snapshot", vec![typed, signed_in]);

    let method = |id: &str| {
        app.accounts_snapshot()
            .accounts
            .into_iter()
            .find(|entry| entry.id == id)
            .expect("the account is listed")
            .sign_in
    };
    assert_eq!(method(&typed_id), SignInMethod::Password);
    assert_eq!(method(&signed_in_id), SignInMethod::Provider);
}

#[test]
fn a_password_that_does_not_connect_leaves_the_account_signed_in_through_its_provider() {
    let (config, id) = imap_account("keeps-grant", true);
    let (app, recorder) = booted("sign-in-password-refused", vec![config]);

    let switched = app.switch_account_to_password(id.clone(), "typed".to_owned());

    assert!(switched.is_err(), "nothing answers on the mail server");
    assert!(signs_in_through_its_provider(&app, &id));
    assert!(
        recorder.persisted.lock().unwrap().is_empty(),
        "nothing is stored before the password connects"
    );
}

#[test]
fn an_empty_password_is_refused_before_anything_is_dialled() {
    let (config, id) = imap_account("empty-password", true);
    let (app, _) = booted("sign-in-password-empty", vec![config]);

    assert!(matches!(
        app.switch_account_to_password(id, String::new()),
        Err(MailcalError::Config(_))
    ));
}

#[test]
fn a_setup_for_another_mailbox_is_not_used_for_this_account() {
    let (config, id) = imap_account("this-one", false);
    let (other, _) = imap_account("another-one", true);
    let (app, recorder) = booted("sign-in-other-mailbox", vec![config]);

    assert!(matches!(
        app.use_credential_from(id.clone(), other, None),
        Err(MailcalError::DifferentAccount)
    ));
    assert!(!signs_in_through_its_provider(&app, &id));
    assert!(recorder.persisted.lock().unwrap().is_empty());
}

/// A calendar for `who@example.com`, set up without mail, at a server that does not answer.
fn calendar_account(who: &str, password: &str) -> (String, String) {
    let config = mailcal_account::AccountConfig {
        imap: None,
        smtp: None,
        caldav: Some(mailcal_account::CalDavAccount {
            base_url: "http://127.0.0.1:1/dav".to_owned(),
            username: format!("{who}@example.com"),
            password: Some(mailcal_account::Secret::new(password.to_owned())),
        }),
        carddav: None,
        oauth: None,
        certificate_exceptions: Vec::new(),
        shape: mailcal_account::AccountShape::default(),
        dav_starts: mailcal_account::DavStarts::default(),
    };
    let id = config.account_id().expect("a valid account id");
    (
        config.to_toml().expect("serializable config"),
        id.as_str().to_owned(),
    )
}

#[test]
fn a_calendar_set_up_again_offers_its_password_to_the_calendar_already_there() {
    let (config, id) = calendar_account("calendar", "stored");
    let (again, again_id) = calendar_account("calendar", "typed");
    assert_eq!(id, again_id, "the same calendar");
    let (app, recorder) = booted("sign-in-calendar-again", vec![config]);

    let used = app.use_credential_from(id, again, None);

    assert!(
        matches!(
            used,
            Err(MailcalError::Connect(_) | MailcalError::SignInRefused { .. })
        ),
        "the password is tried against the server, not refused as missing: {used:?}"
    );
    assert!(recorder.persisted.lock().unwrap().is_empty());
}

#[test]
fn signing_in_again_asks_the_stored_issuer_for_the_accounts_own_address() {
    let (config, id) = imap_account("again", true);
    let (app, _) = booted("sign-in-again", vec![config]);

    let start = app
        .begin_account_sign_in(id, "mailcal.test://ignored".to_owned())
        .expect("a stored grant needs no discovery");
    let url = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    assert_eq!(
        format!("{}{}", url.origin().ascii_serialization(), url.path()),
        "http://127.0.0.1:1/authorize",
        "the issuer the account signed in at"
    );
    let param = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    assert_eq!(param("login_hint").as_deref(), Some("again@example.com"));
    assert_eq!(
        param("client_id").as_deref(),
        Some("client-abc"),
        "the client the account registered, not a second one"
    );
    assert_eq!(
        param("redirect_uri").as_deref(),
        Some("mailcal.test://imap-oauth"),
        "the redirect the grant was issued for"
    );
}

#[test]
fn a_sign_in_the_server_issued_for_another_account_is_refused() {
    assert!(named_as(None, "alice@example.org").is_ok());
    assert!(named_as(Some("Alice@Example.org"), "alice@example.org").is_ok());
    assert!(matches!(
        named_as(Some("bob@example.org"), "alice@example.org"),
        Err(MailcalError::DifferentAccount)
    ));
}

#[test]
fn an_unknown_account_is_offered_no_switch() {
    let (app, _) = booted("sign-in-unknown", Vec::new());
    let options = app.account_sign_in_options("nobody@example.com@nowhere".to_owned());
    assert!(!options.sign_in && !options.password);
}

#[test]
fn an_imap_sign_in_is_remedied_by_signing_in_again_and_a_password_by_typing_one() {
    let (typed, typed_id) = imap_account("remedy-typed", false);
    let (signed_in, signed_in_id) = imap_account("remedy-signed-in", true);
    let (app, _) = booted("sign-in-remedy", vec![typed, signed_in]);

    assert!(matches!(
        app.account_provider(typed_id),
        Some(crate::AccountProvider::Password)
    ));
    assert!(matches!(
        app.account_provider(signed_in_id),
        Some(crate::AccountProvider::ImapOauth)
    ));
}
