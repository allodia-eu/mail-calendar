//! What a separate server's own sign-in refuses before anything is asked of a provider. A sign-in
//! that completes is the live suite's (`tests/live_calendar_sign_in.rs`).

use std::sync::{Arc, mpsc};

use crate::{
    AccountCapability, LogLevel, MailcalApp, MailcalError,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const MAILBOX: &str = r#"
[imap]
addr = "127.0.0.1:1"
server_name = "127.0.0.1"
username = "alice@example.com"
password = "typed"

[caldav]
base_url = "https://127.0.0.1:1/dav"
username = "alice@example.com"
password = "typed"
"#;

fn booted(name: &str) -> (Arc<MailcalApp>, String) {
    let config = mailcal_account::load_str(MAILBOX).unwrap();
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config.to_toml().unwrap()],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots with placeholders for accounts it cannot reach");
    let id = config.account_id().unwrap().as_str().to_owned();
    (app, id)
}

#[test]
fn a_server_that_does_not_answer_offers_no_sign_in() {
    let (app, id) = booted("dav-oauth-silent");
    let both = vec![AccountCapability::Calendar, AccountCapability::Contacts];

    assert!(!app.separate_server_offers_sign_in(id.clone(), both.clone()));
    let begun = app.begin_separate_server_sign_in(id, both, "mailcal.test://sign-in".to_owned());
    assert!(
        matches!(begun, Err(MailcalError::Connect(_))),
        "{:?}",
        begun.err()
    );
}

#[test]
fn only_a_mailbox_s_calendar_or_contacts_can_be_signed_in_to() {
    let (app, id) = booted("dav-oauth-nothing");

    assert!(!app.separate_server_offers_sign_in(id.clone(), vec![AccountCapability::Mail]));
    assert!(!app.separate_server_offers_sign_in(
        "nobody@example.com".to_owned(),
        vec![AccountCapability::Calendar]
    ));
    let begun = app.begin_separate_server_sign_in(
        id,
        vec![AccountCapability::Mail],
        "mailcal.test://sign-in".to_owned(),
    );
    assert!(
        matches!(begun, Err(MailcalError::Config(_))),
        "{:?}",
        begun.err()
    );
}

/// A sign-in begun for `account_id`, whose redirect is expected to echo the state `s`.
fn pending_for(account_id: &str) -> String {
    serde_json::json!({
        "account_id": account_id,
        "calendar": true,
        "contacts": false,
        "login": "alice@example.com",
        "client_id": "c",
        "client_secret": null,
        "authorize_endpoint": "https://127.0.0.1:1/authorize",
        "token_endpoint": "https://127.0.0.1:1/token",
        "redirect_uri": "mailcal.test://sign-in",
        "scopes": ["offline_access"],
        "resource": null,
        "issuer": null,
        "authorization_server": "https://127.0.0.1:1",
        "protected_resources": [],
        "state": "s",
        "verifier": "v",
    })
    .to_string()
}

#[test]
fn a_sign_in_started_for_another_account_is_not_completed() {
    let (app, id) = booted("dav-oauth-other");
    let pending = pending_for("someone-else@example.com");

    let completed = app.complete_separate_server_sign_in(
        id,
        pending,
        "mailcal.test://sign-in?code=c&state=s".to_owned(),
    );

    assert!(
        matches!(completed, Err(MailcalError::Config(_))),
        "{:?}",
        completed.err()
    );
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);
}

#[test]
fn a_redirect_from_another_sign_in_is_named_and_its_code_never_exchanged() {
    let (app, id) = booted("dav-oauth-stale");

    // The token endpoint is unreachable, so reaching it would answer `Connect` instead.
    let completed = app.complete_separate_server_sign_in(
        id.clone(),
        pending_for(&id),
        "mailcal.test://sign-in?code=c&state=an-older-request".to_owned(),
    );

    assert!(
        matches!(completed, Err(MailcalError::StaleSignIn)),
        "{:?}",
        completed.err()
    );
    assert_eq!(app.accounts_snapshot().accounts.len(), 1);
}

fn issuer(issuer: &str, protected: &[&str], scopes: &[&str]) -> mailcal_oauth::AuthServerMetadata {
    mailcal_oauth::AuthServerMetadata {
        issuer: issuer.to_owned(),
        authorization_endpoint: format!("{issuer}/authorize"),
        token_endpoint: format!("{issuer}/token"),
        registration_endpoint: Some(format!("{issuer}/register")),
        revocation_endpoint: None,
        userinfo_endpoint: None,
        scopes_supported: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
        end_session_endpoint: None,
        prompt_values_supported: Vec::new(),
        issuer_parameter_supported: true,
        protected_resources: protected.iter().map(|entry| (*entry).to_owned()).collect(),
    }
}

const BOTH: [mailcal_oauth::DavUse; 2] = [
    mailcal_oauth::DavUse::Calendars,
    mailcal_oauth::DavUse::Contacts,
];

const DAV_SCOPES: [&str; 3] = [
    "offline_access",
    "urn:ietf:params:oauth:scope:calendars",
    "urn:ietf:params:oauth:scope:contacts",
];

#[test]
fn an_issuer_on_the_calendar_servers_site_signs_it_in() {
    let hosts = ["dav.example.com".to_owned()];

    let scopes = super::usable(
        &issuer("https://auth.example.com", &[], &DAV_SCOPES),
        &hosts,
        &BOTH,
    );

    assert_eq!(
        scopes.as_deref(),
        Ok(DAV_SCOPES.map(str::to_owned).as_slice())
    );
}

/// The server names the issuer, but only the issuer says where its tokens go: a calendar server
/// on another site naming a provider's sign-in gets no token from it.
#[test]
fn an_issuer_whose_tokens_are_not_for_the_server_is_refused() {
    let hosts = ["dav.example.com".to_owned()];

    assert!(
        super::usable(
            &issuer("https://accounts.elsewhere.net", &[], &DAV_SCOPES),
            &hosts,
            &BOTH
        )
        .is_err()
    );
    assert!(
        super::usable(
            &issuer(
                "https://auth.example.com",
                &["https://mail.example.com"],
                &DAV_SCOPES
            ),
            &hosts,
            &BOTH
        )
        .is_err(),
        "an issuer that lists its resources reaches only those"
    );
    assert!(
        super::usable(
            &issuer(
                "https://accounts.elsewhere.net",
                &["https://dav.example.com/dav"],
                &DAV_SCOPES
            ),
            &hosts,
            &BOTH
        )
        .is_ok(),
        "an issuer elsewhere that lists the server reaches it"
    );
}

#[test]
fn an_issuer_must_register_apps_and_offer_what_moves() {
    let hosts = ["dav.example.com".to_owned()];
    let mut closed = issuer("https://auth.example.com", &[], &DAV_SCOPES);
    closed.registration_endpoint = None;

    assert!(super::usable(&closed, &hosts, &BOTH).is_err());
    assert!(
        super::usable(
            &issuer(
                "https://auth.example.com",
                &[],
                &["offline_access", "caldav"]
            ),
            &hosts,
            &BOTH
        )
        .is_err()
    );
}
