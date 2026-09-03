//! Tests for the registry's three load-bearing properties: an unregistered account cannot be
//! dialed, a rotation lands where the persist will read it, and nothing can put a stale config
//! back.
//!
//! These are unit tests over the registry alone. The same properties end to end; through the real
//! FFI constructors, over a token endpoint that actually rotates, are in
//! `crate::tests_credential_ordering`, because a property that holds in a unit test and not in the
//! boot path is the exact shape of the bug this all came from.

use mailcal_account::{
    AccountError, GoogleConfig, JmapAccountConfig, MicrosoftConfig, OAuthGrant, Secret,
};
use provider_imap::ImapError;

use super::{AccountRegistry, JmapLookup, Rotation, dial::ConnectFailure};
use crate::ConnectedAccount;

fn jmap_entry(refresh: &str) -> (String, ConnectedAccount) {
    let config = JmapAccountConfig {
        email: "alice@example.com".to_owned(),
        base_url: "https://api.example.com".to_owned(),
        password: None,
        token: None,
        oauth: Some(OAuthGrant {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: Secret::new(refresh.to_owned()),
            authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
            issuer: None,
        }),
    };
    let id = config
        .account_id()
        .expect("a valid account id")
        .as_str()
        .to_owned();
    (
        id,
        ConnectedAccount::Jmap {
            config,
            tokens: None,
        },
    )
}

fn account_id(id: &str) -> engine_api::AccountId {
    engine_api::AccountId::try_from(id).expect("a valid account id")
}

/// The runtime half of the type-level rule: no entry, no dial.
///
/// The compile-time half cannot be written as a test; `AccountDial::from_entry` is `pub(super)`,
/// so a module outside the registry that tries to build one does not compile, and a test that does
/// not compile is not a test. What *is* checkable is that the only public door refuses.
#[test]
fn an_unregistered_account_cannot_be_dialed() {
    let registry = AccountRegistry::new();

    assert!(
        registry
            .dial("nobody@example.com@imap.example.com")
            .is_none()
    );
    assert!(!registry.contains("nobody@example.com@imap.example.com"));
}

/// A rotation during a dial advances the entry, and the bytes the store is given come from that
/// advanced entry: not from whatever the caller parsed a moment earlier.
///
/// This is the pair that came apart and cost a real account its grant: the registry always
/// advanced, and the *store* got a config serialized before the rotation.
#[test]
fn a_rotation_advances_the_entry_the_persist_reads_from() {
    let registry = AccountRegistry::new();
    let (id, entry) = jmap_entry("original-refresh");
    let _registered = registry.pre_register(id.clone(), entry);

    let rotation = registry.rotate_refresh_token(&account_id(&id), "rotated-refresh");

    let Rotation::Advanced {
        family,
        config_toml,
    } = rotation
    else {
        panic!("an OAuth JMAP account's rotation must advance its entry");
    };
    assert_eq!(family, "jmap");
    // What the sink hands the store...
    assert!(config_toml.contains("rotated-refresh"));
    // ...is the same thing a later `persist_registered_grant` would read.
    let from_registry = registry
        .oauth_config_toml(&id)
        .expect("a registered OAuth account");
    let parsed = mailcal_account::load_jmap_str(&from_registry).expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-refresh",
        "the registry kept the token the server replaced, so a persist would write a spent one",
    );
}

/// Attaching the live token source after a dial cannot touch the config, which is what makes the
/// connect-before-register trap unexpressible rather than merely documented.
///
/// The old API took a whole `ConnectedAccount` back after the connect, and two paths passed the
/// config their connect had parsed from the original TOML. That silently undid a rotation the sink
/// had already persisted, so the registry and the store disagreed for the rest of the session and
/// the next launch presented a replay. There is no longer a method that could do it.
#[test]
fn committing_after_a_dial_cannot_put_a_stale_config_back() {
    let registry = AccountRegistry::new();
    let (id, entry) = jmap_entry("original-refresh");
    let registered = registry.pre_register(id.clone(), entry);
    registry.rotate_refresh_token(&account_id(&id), "rotated-mid-dial");

    // The successful path consumes only the rollback token; the registered entry is untouched.
    registered.commit();

    let parsed = mailcal_account::load_jmap_str(
        &registry
            .oauth_config_toml(&id)
            .expect("a registered OAuth account"),
    )
    .expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "rotated-mid-dial",
    );
}

/// A rotation for an account nobody registered is reported as such, so the sink can shout.
///
/// Reachable only from a bug now, which is exactly why it must stay distinguishable from "this
/// account has nothing to rotate": one is a lost credential and the other is a no-op.
#[test]
fn a_rotation_with_no_entry_is_reported_as_unregistered() {
    let registry = AccountRegistry::new();

    let rotation =
        registry.rotate_refresh_token(&account_id("ghost@example.com@api.example.com"), "new");

    assert!(matches!(rotation, Rotation::Unregistered));
}

/// A password account has nothing to rotate: silent, not an error, and never confused with a lost
/// credential.
#[test]
fn a_password_account_has_nothing_to_rotate() {
    let registry = AccountRegistry::new();
    let config = mailcal_account::load_str(
        &crate::account_config_toml(crate::AccountSetup {
            imap_host: "imap.example.com".to_owned(),
            username: "bob@example.com".to_owned(),
            password: "pw".to_owned(),
            smtp_host: None,
            caldav_base_url: None,
            imap_security: None,
            smtp_security: None,
        })
        .expect("a valid account config"),
    )
    .expect("a valid IMAP config");
    let id = config
        .account_id()
        .expect("a valid account id")
        .as_str()
        .to_owned();
    let _registered = registry.pre_register(
        id.clone(),
        ConnectedAccount::Imap {
            config,
            tokens: None,
        },
    );

    let rotation = registry.rotate_refresh_token(&account_id(&id), "new");

    assert!(matches!(
        rotation,
        Rotation::Nothing {
            encode_error: None,
            family: "imap",
        }
    ));
    assert!(
        registry.oauth_config_toml(&id).is_err(),
        "a password account is stored as the host supplied it, never re-serialized from here",
    );
}

#[test]
fn replacement_credentials_are_built_for_password_and_secret_jmap_accounts_only() {
    let registry = AccountRegistry::new();
    let imap = mailcal_account::load_str(
        &crate::account_config_toml(crate::AccountSetup {
            imap_host: "imap.example.com".to_owned(),
            username: "bob@example.com".to_owned(),
            password: "old-imap".to_owned(),
            smtp_host: Some("smtp.example.com".to_owned()),
            caldav_base_url: Some("https://dav.example.com".to_owned()),
            imap_security: None,
            smtp_security: None,
        })
        .expect("a valid account config"),
    )
    .expect("a valid IMAP config");
    let imap_id = imap.account_id().unwrap().as_str().to_owned();
    let _imap = registry.pre_register(
        imap_id.clone(),
        ConnectedAccount::Imap {
            config: imap,
            tokens: None,
        },
    );

    let jmap_config = JmapAccountConfig {
        email: "jane@example.com".to_owned(),
        base_url: "https://api.example.com".to_owned(),
        password: None,
        token: Some(Secret::new("legacy-token".to_owned())),
        oauth: None,
    };
    let jmap_id = jmap_config.account_id().unwrap().as_str().to_owned();
    let _jmap = registry.pre_register(
        jmap_id.clone(),
        ConnectedAccount::Jmap {
            config: jmap_config,
            tokens: None,
        },
    );
    let (oauth_id, oauth) = jmap_entry("refresh-token");
    let _oauth = registry.pre_register(oauth_id.clone(), oauth);

    let imap_toml = registry
        .replacement_secret_toml(&imap_id, "new-imap")
        .expect("IMAP passwords can be replaced");
    let parsed_imap = mailcal_account::load_str(&imap_toml).expect("valid IMAP TOML");
    assert_eq!(
        parsed_imap.imap.password.as_ref().unwrap().expose(),
        "new-imap"
    );
    assert_eq!(
        parsed_imap.caldav.unwrap().password.unwrap().expose(),
        "new-imap"
    );

    let jmap_toml = registry
        .replacement_secret_toml(&jmap_id, "new-jmap")
        .expect("stored JMAP secrets can be replaced");
    let parsed_jmap = mailcal_account::load_jmap_str(&jmap_toml).expect("valid JMAP TOML");
    assert_eq!(parsed_jmap.password.unwrap().expose(), "new-jmap");
    assert!(
        parsed_jmap.token.is_none(),
        "new secrets use the negotiated field"
    );

    assert!(
        registry
            .replacement_secret_toml(&oauth_id, "wrong")
            .is_err()
    );
    assert!(
        registry
            .replacement_secret_toml("missing", "wrong")
            .is_err()
    );
}

/// A rollback restores what it displaced, rather than deleting a live account.
///
/// The re-authentication paths are why: the host re-runs sign-in for an account it already has, so
/// an entry is routinely there, and a rollback that merely removed would take a working account's
/// re-connection state away because its *re-auth* failed.
#[test]
fn a_rollback_restores_the_entry_it_displaced() {
    let registry = AccountRegistry::new();
    let (id, original) = jmap_entry("the-live-grant");
    let _first = registry.pre_register(id.clone(), original);
    let (_, replacement) = jmap_entry("the-new-grant");

    let registered = registry.pre_register(id.clone(), replacement);
    registered.rollback(&registry);

    let parsed = mailcal_account::load_jmap_str(
        &registry
            .oauth_config_toml(&id)
            .expect("the displaced account is back"),
    )
    .expect("valid config TOML");
    assert_eq!(
        parsed.oauth.expect("an oauth grant").refresh_token.expose(),
        "the-live-grant",
        "a failed re-authentication deleted the account it was repairing",
    );
}

/// A rollback of a *first* registration removes it, so a failed add leaves nothing behind.
#[test]
fn a_rollback_of_a_new_account_removes_it() {
    let registry = AccountRegistry::new();
    let (id, entry) = jmap_entry("original-refresh");

    registry.pre_register(id.clone(), entry).rollback(&registry);

    assert!(!registry.contains(&id));
}

/// All three OAuth families rotate through the same door, and each is named for the log.
#[test]
fn every_oauth_family_rotates_and_names_itself() {
    let registry = AccountRegistry::new();
    let microsoft = MicrosoftConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        tenant: "common".to_owned(),
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("original".to_owned()),
    };
    let google = GoogleConfig {
        email: "alice@example.com".to_owned(),
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://auth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        refresh_token: Secret::new("original".to_owned()),
    };
    let microsoft_id = microsoft.account_id().expect("a valid id");
    let google_id = google.account_id().expect("a valid id");
    let tokens = mailcal_account::GraphTokenSource::new(
        &microsoft,
        microsoft_id.clone(),
        None,
        mailcal_account::CredentialOrigin::FreshSignIn,
    )
    .expect("a token source over a well-formed config");
    let _m = registry.pre_register(
        microsoft_id.as_str().to_owned(),
        ConnectedAccount::Microsoft {
            config: microsoft,
            tokens: std::sync::Arc::clone(&tokens),
        },
    );
    let _g = registry.pre_register(
        google_id.as_str().to_owned(),
        ConnectedAccount::Google {
            config: google,
            tokens,
        },
    );

    for (id, expected) in [(&microsoft_id, "graph"), (&google_id, "google")] {
        let Rotation::Advanced {
            family,
            config_toml,
        } = registry.rotate_refresh_token(id, "rotated")
        else {
            panic!("{expected} must rotate");
        };
        assert_eq!(family, expected);
        assert!(config_toml.contains("rotated"));
    }
    assert_eq!(registry.protocols().len(), 2);
    assert_eq!(registry.account_type(microsoft_id.as_str()), "graph");
}

/// The registry answers the two family questions a host asks it, and says "unknown" rather than
/// panicking for an account that is gone.
#[test]
fn family_lookups_tolerate_an_account_that_has_been_removed() {
    let registry = AccountRegistry::new();
    let (id, entry) = jmap_entry("original-refresh");
    let _registered = registry.pre_register(id.clone(), entry);
    assert!(registry.provider(&id).is_some());
    assert!(registry.imap_config(&id).is_none(), "JMAP has no IMAP half");
    assert!(registry.jmap_config(&id).is_ok());

    registry.remove(&id);

    assert_eq!(registry.account_type(&id), "unknown");
    assert!(registry.provider(&id).is_none());
    assert!(matches!(
        registry.jmap_config(&id),
        Err(JmapLookup::Unknown)
    ));
}

/// The verdict `reconnect_all` branches on: a refused credential raises the "sign in again" prompt,
/// everything else badges an outage (`docs/provider-oauth.md` rule 12). Whose credential: an OAuth
/// grant's or a password's; must not enter into it.
#[test]
fn a_refused_credential_of_any_family_is_not_an_outage() {
    for err in [
        AccountError::SigninRejected("token expired".to_owned()),
        AccountError::SigninRejected(
            "imap: IMAP authentication failed: [AUTHENTICATIONFAILED]".to_owned(),
        ),
    ] {
        let failure = ConnectFailure::from(err);
        assert!(failure.signin_expired(), "badged as an outage: {failure}");
    }
}

/// The other direction, which is the one a wrong guess costs a user most: a prompt they cannot
/// dismiss and did not earn. Only a refusal counts; including an IMAP auth failure that arrives by
/// the ordinary conversion, i.e. from a folder login the INBOX had already contradicted.
#[test]
fn nothing_short_of_a_refused_credential_asks_for_a_new_signin() {
    for err in [
        AccountError::Imap(ImapError::Auth("[AUTHENTICATIONFAILED]".to_owned())),
        AccountError::Imap(ImapError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "refused",
        ))),
        AccountError::Jmap("JMAP HTTP 503: unavailable".to_owned()),
        AccountError::Graph("token refresh: timed out".to_owned()),
        AccountError::MailboxList("LIST failed".to_owned()),
    ] {
        let failure = ConnectFailure::from(err);
        assert!(
            !failure.signin_expired(),
            "asked the user to sign in again over: {failure}"
        );
    }
}

/// The detail the outage badge and the log read is the error itself: a failure that renders as
/// nothing tells a support session nothing.
#[test]
fn a_connect_failure_renders_the_cause_it_was_built_from() {
    let failure = ConnectFailure::from(AccountError::Jmap("JMAP HTTP 503: unavailable".to_owned()));
    assert!(failure.to_string().contains("503"), "{failure}");
}
