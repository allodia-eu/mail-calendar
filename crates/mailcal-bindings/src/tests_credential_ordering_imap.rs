//! [`super`]'s property for an IMAP account that signs in through its provider: its renewed
//! refresh token reaches the host's store like any other grant's.

use std::{
    fs,
    sync::{Arc, atomic::Ordering, mpsc},
};

use super::{rotating_account, rotating_token_endpoint};
use crate::{
    LogLevel, MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// An IMAP account signed in through the provider, whose token endpoint is `endpoint` and whose
/// mail host refuses.
fn rotating_imap_account(who: &str, endpoint: String) -> (String, engine_api::AccountId) {
    let (jmap, _) = rotating_account(who, endpoint);
    let jmap = mailcal_account::load_jmap_str(&jmap).expect("valid config TOML");
    let config = mailcal_account::AccountConfig {
        imap: Some(mailcal_account::ImapAccount {
            addr: "127.0.0.1:1".to_owned(),
            server_name: "127.0.0.1".to_owned(),
            username: format!("{who}@example.com"),
            password: None,
            security: mailcal_account::ConnectionSecurity::ImplicitTls,
        }),
        smtp: None,
        caldav: None,
        carddav: None,
        oauth: jmap.oauth,
        certificate_exceptions: Vec::new(),
        shape: mailcal_account::AccountShape::default(),
        dav_starts: mailcal_account::DavStarts::default(),
    };
    let id = config.account_id().expect("a valid account id");
    (config.to_toml().expect("serializable config"), id)
}

/// The IMAP half of [`assert_rotation_reached_the_store`].
fn assert_imap_rotation_reached_the_store(
    recorder: &RecordingCredentialStore,
    expected_id: &engine_api::AccountId,
    context: &str,
) {
    let written = recorder.persisted.lock().expect("recorder mutex poisoned");
    let (persisted_id, toml) = written
        .last()
        .unwrap_or_else(|| panic!("{context}: nothing reached the store"));
    assert_eq!(persisted_id, expected_id.as_str());
    let parsed = mailcal_account::load_str(toml).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-1",
        "{context}: the stored config carries the token the renewal replaced",
    );
}

#[test]
fn a_renewal_of_an_imap_sign_in_reaches_the_store() {
    let (config_toml, account_id) =
        rotating_imap_account("imap-renewed", "http://127.0.0.1:1/token".to_owned());
    let data_dir = temp_data_dir("imap-rotation");
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config_toml],
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("app boots with a provider-less placeholder");

    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    app.runtime.block_on(|| async {
        sink.refresh_token_rotated(&account_id, "rotated-1").await;
    });
    assert_imap_rotation_reached_the_store(&recorder, &account_id, "a booted IMAP sign-in");

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn a_renewal_while_an_imap_sign_in_is_added_reaches_the_store() {
    let (endpoint, refreshes) = rotating_token_endpoint("original-refresh");
    let (config_toml, account_id) = rotating_imap_account("imap-added", endpoint);
    let data_dir = temp_data_dir("imap-add-rotation");
    let recorder = Arc::new(RecordingCredentialStore::default());
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&recorder))),
    )
    .expect("an account-less app boots");

    let added = app.add_account(config_toml);

    assert!(added.is_err(), "nothing is listening on the mail host");
    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "the add never refreshed, so this test is not exercising a renewal at all",
    );
    assert_imap_rotation_reached_the_store(&recorder, &account_id, "add_account");

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// The other half of an add's store write, which only a connect that succeeds reaches: an IMAP
/// sign-in's stored bytes come from the registry, which a renewal during the connect advanced.
/// The harness's server renews no token on a first refresh, so no live test can tell the two
/// apart; this pins the decision.
#[test]
fn only_a_password_imap_account_is_stored_as_the_host_passed_it() {
    let (signed_in, _) =
        rotating_imap_account("imap-decided", "http://127.0.0.1:1/token".to_owned());
    let mut config = mailcal_account::load_str(&signed_in).expect("valid config TOML");
    config.oauth = None;
    let with_password = config
        .with_password("typed")
        .to_toml()
        .expect("serializable config");
    let (tx, _rx) = mpsc::channel();
    let data_dir = temp_data_dir("imap-stored-as-passed");
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::default())),
    )
    .expect("an account-less app boots");
    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    let prepared = |toml: &str| {
        crate::boot::prepare_stored_account(
            toml,
            &sink,
            mailcal_account::CredentialOrigin::FreshSignIn,
        )
        .expect("a well-formed config prepares")
        .connected
    };
    assert!(!prepared(&signed_in).stores_a_password());
    assert!(prepared(&with_password).stores_a_password());

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}
