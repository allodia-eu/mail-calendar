//! Credential-store and account-registration **ordering** tests, over the real FFI constructors.
//!
//! Their own file rather than `tests.rs` because they assert one thing: that a rotated credential
//! always has somewhere to go: a host store wired before the first refresh can happen, and a
//! registry entry present before the connect that might rotate it. See
//! [`crate::credential_store`] and `crate::account_registry`.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, mpsc},
};

use crate::{
    AccountCredentialStore, LogLevel, MailcalApp,
    account_registry::Registered,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// Registers a JMAP config the way every add path does; parse, build the one token source, write
/// the entry, and hands back the token, so a test can drive the ordering without a network.
fn register_jmap(app: &MailcalApp, config_toml: &str) -> Registered {
    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    let prepared = crate::boot::prepare_stored_account(
        config_toml,
        &sink,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("a well-formed config prepares");
    app.registry
        .pre_register(prepared.account.id.as_str().to_owned(), prepared.connected)
}

/// The host's credential store is wired into the token sink **by the constructor**, so a
/// rotation is persistable from the first instant the core can refresh anything.
///
/// This is the boot half of `docs/provider-oauth.md` rule 5, and the ordering it locks is not
/// theoretical: `build_accounts` starts the background dial in its *last statement*, and a
/// production Android launch measured the first OAuth refresh beginning 6 ms later; while the
/// host was still blocked inside the constructor. When the store arrived through a setter
/// instead, two of the four clients installed it from a UI-thread post, so a rotation landing
/// half a second later was saved or dropped depending on whether the main thread had had a turn.
/// A dropped one is silent for an hour and then revokes the grant on a ratcheting server.
///
/// The account here is deliberately unreachable (`127.0.0.1:1`): the point is what the sink is
/// wired to when the constructor returns, not whether the dial succeeds.
#[test]
fn the_constructors_credential_store_is_live_before_the_first_refresh_can_be() {
    let (tx, _rx) = mpsc::channel();
    let grant = mailcal_account::JmapOAuth {
        client_id: "client-abc".to_owned(),
        client_secret: None,
        refresh_token: mailcal_account::Secret::new("original-refresh".to_owned()),
        authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
        token_endpoint: "http://127.0.0.1:1/token".to_owned(),
        redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        resource: None,
    };
    let config = mailcal_account::JmapAccountConfig {
        email: "rotating@example.com".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        password: None,
        token: None,
        oauth: Some(grant),
    };
    let account_id = config.account_id().expect("a valid account id");
    let config_toml = config.to_toml().expect("serializable config");
    let data_dir = temp_data_dir("boot-credential-store");
    let recorder = Arc::new(RecordingCredentialStore::default());

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

    // The sink the core's own refreshes report through, assembled exactly as every path inside
    // the app assembles it. Nothing installed a store after construction; there is no longer a
    // way to.
    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    app.runtime.block_on(async {
        sink.refresh_token_rotated(&account_id, "rotated-refresh")
            .await;
    });

    let written = recorder.persisted.lock().expect("recorder mutex poisoned");
    let (persisted_id, toml) = written
        .first()
        .expect("the store handed to the constructor received the rotation");
    assert_eq!(persisted_id, account_id.as_str());
    let parsed = mailcal_account::load_jmap_str(toml).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-refresh",
        "the persisted config carries the NEW token, not the one it replaced",
    );

    drop(written);
    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// A `[jmap]` config with an OAuth grant, pointed at a port nothing listens on so any connect
/// fails immediately. Shared by the two `add_account` ordering tests below.
fn unreachable_oauth_jmap_config() -> (String, mailcal_account::JmapAccountConfig) {
    let config = mailcal_account::JmapAccountConfig {
        email: "rotating@example.com".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        password: None,
        token: None,
        oauth: Some(mailcal_account::JmapOAuth {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: mailcal_account::Secret::new("original-refresh".to_owned()),
            authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
            token_endpoint: "http://127.0.0.1:1/token".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
        }),
    };
    (config.to_toml().expect("serializable config"), config)
}

/// Builds an account-less app over a throwaway store, for the `add_account` ordering tests.
fn app_with_recorder(name: &str) -> (Arc<MailcalApp>, Arc<RecordingCredentialStore>, PathBuf) {
    let (tx, _rx) = mpsc::channel();
    let data_dir = temp_data_dir(name);
    let recorder = Arc::new(RecordingCredentialStore::default());
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
    (app, recorder, data_dir)
}

/// An `add_account` whose connect fails leaves nothing behind. Registering before connecting is
/// what makes a rollback necessary at all: without it the registry would keep an entry for an
/// account the app never got.
#[test]
fn a_failed_add_account_leaves_no_registry_entry_behind() {
    let (app, _recorder, data_dir) = app_with_recorder("add-account-rollback");
    let (config_toml, config) = unreachable_oauth_jmap_config();
    let account_id = config.account_id().expect("a valid account id");

    let result = app.add_account(config_toml);

    assert!(result.is_err(), "nothing is listening on 127.0.0.1:1");
    assert!(
        !app.registry.contains(account_id.as_str()),
        "the pre-registered entry is rolled back when the connect fails",
    );

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn a_rejected_replacement_keeps_the_registered_and_stored_password() {
    let (app, recorder, data_dir) = app_with_recorder("password-repair-rollback");
    let config_toml = crate::account_config_toml(crate::AccountSetup {
        imap_host: "127.0.0.1:1".to_owned(),
        username: "repair@example.com".to_owned(),
        password: "old-password".to_owned(),
        smtp_host: None,
        caldav_base_url: None,
        imap_security: None,
        smtp_security: None,
    })
    .expect("valid account config");
    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    let prepared = crate::boot::prepare_stored_account(
        &config_toml,
        &sink,
        mailcal_account::CredentialOrigin::Stored,
    )
    .expect("stored account prepares");
    let account_id = prepared.account.id.as_str().to_owned();
    let _registered = app
        .registry
        .pre_register(account_id.clone(), prepared.connected);

    let result = app.replace_account_secret(account_id.clone(), "mistyped-password".to_owned());

    assert!(
        result.is_err(),
        "nothing is listening on the candidate endpoint"
    );
    assert_eq!(
        app.registry
            .imap_config(&account_id)
            .expect("the displaced entry was restored")
            .imap
            .password
            .expose(),
        "old-password",
    );
    assert!(
        recorder
            .persisted
            .lock()
            .expect("recorder mutex poisoned")
            .is_empty(),
        "a candidate the server refused must never replace the durable credential",
    );

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// What the core writes to the store is the config the **registry** holds, not the TOML the
/// caller passed in.
///
/// This is the whole reason the core took the write over from the clients. A connect refreshes,
/// a refresh can rotate, and the caller's string was serialized before any of that: so a client
/// persisting the config it had been handed at sign-in was writing the token the rotation had
/// already replaced. On a ratcheting server, presenting that superseded token later revokes the
/// grant, which is how a real JMAP account died.
#[test]
fn the_stored_credential_is_the_registrys_config_not_the_callers() {
    let (app, recorder, data_dir) = app_with_recorder("persist-from-registry");
    let (config_toml, config) = unreachable_oauth_jmap_config();
    let account_id = config.account_id().expect("a valid account id");

    let _registered = register_jmap(&app, &config_toml);
    // A rotation lands mid-connect, exactly as it does on a real dial.
    let sink = crate::token_sink::token_sink(&app.registry, &app.credential_store);
    app.runtime.block_on(async {
        sink.refresh_token_rotated(&account_id, "rotated-mid-connect")
            .await;
    });
    recorder
        .persisted
        .lock()
        .expect("recorder mutex poisoned")
        .clear();

    // What `add_account` does once the connect returns.
    app.persist_registered_grant(account_id.as_str())
        .expect("the recording store accepts the write");

    let written = recorder.persisted.lock().expect("recorder mutex poisoned");
    let (_, toml) = written.first().expect("the add wrote the credential");
    let parsed = mailcal_account::load_jmap_str(toml).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-mid-connect",
        "persisting the caller's TOML would have written the token the rotation replaced",
    );

    drop(written);
    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// Removing an account erases its stored credential: the core's job now, not a second call the
/// host has to remember to make after `remove_account`.
///
/// Every client made both calls in sequence, so an account could be gone from the app while its
/// credential stayed in the vault, and the next launch brought it back with nothing to explain it.
#[test]
fn removing_an_account_erases_its_stored_credential() {
    let (app, recorder, data_dir) = app_with_recorder("remove-erases-credential");
    let (config_toml, config) = unreachable_oauth_jmap_config();
    let account_id = config.account_id().expect("a valid account id");
    let _registered = register_jmap(&app, &config_toml);

    app.remove_account(account_id.as_str().to_owned())
        .expect("the recording store accepts the erase");

    assert_eq!(
        *recorder.deleted.lock().expect("recorder mutex poisoned"),
        vec![account_id.as_str().to_owned()],
        "the account's credential is erased through the same port it was written through",
    );
    assert!(!app.registry.contains(account_id.as_str()),);

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// A store that refuses is *reported*, not swallowed; in both directions.
///
/// The port used to return nothing at all, so the core logged `re-persisted a rotated refresh
/// token to the host store` whether or not a byte had been written. These are the two answers
/// that line could not give: an add whose credential cannot be stored fails rather than handing
/// the user an account that disappears at the next launch, and a removal whose credential cannot
/// be erased says so rather than leaving a zombie to come back.
#[test]
fn a_store_that_refuses_fails_the_add_and_reports_the_erase() {
    let (tx, _rx) = mpsc::channel();
    let data_dir = temp_data_dir("refusing-store");
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        data_dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RefusingStore),
    )
    .expect("an account-less app boots");
    let (config_toml, config) = unreachable_oauth_jmap_config();
    let account_id = config.account_id().expect("a valid account id");

    let registered = register_jmap(&app, &config_toml);
    let persisted = app.persist_registered_grant(account_id.as_str());
    assert!(
        persisted.is_err(),
        "a refused write must reach the caller, which is what lets the add roll back",
    );
    // What `add_account` does with that error.
    registered.rollback(&app.registry);
    assert!(
        !app.registry.contains(account_id.as_str()),
        "an account whose credential cannot be stored is not left half-added",
    );

    let removed = app.remove_account(account_id.as_str().to_owned());
    assert!(
        removed.is_err(),
        "a refused erase is surfaced: what survives is a credential with no account",
    );

    drop(app);
    let _ = fs::remove_dir_all(data_dir);
}

/// A host store that refuses everything, standing in for a locked Keychain or an Android Keystore
/// key invalidated by a biometric enrolment.
struct RefusingStore;

impl AccountCredentialStore for RefusingStore {
    fn persist(
        &self,
        _account_id: String,
        _config_toml: String,
    ) -> Result<(), crate::CredentialStoreError> {
        Err(crate::CredentialStoreError::Store("locked".to_owned()))
    }

    fn delete(&self, _account_id: String) -> Result<(), crate::CredentialStoreError> {
        Err(crate::CredentialStoreError::Store("locked".to_owned()))
    }
}
