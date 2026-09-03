//! Tests for [`AccountConfig`]: what a stored account must round-trip, and the two
//! credential shapes it can carry.
//!
//! Split from the module, which is at the size limit. Everything here is about the stored
//! form: an account that loses a field at rest fails at the next launch rather than at the
//! edit that caused it, which is the slowest kind of failure to trace back.

use super::*;

const SAMPLE: &str = r#"
[imap]
addr = "imap.soverin.net:993"
server_name = "imap.soverin.net"
username = "you@example.com"
password = "hunter2"

[smtp]
addr = "smtp.soverin.net:465"
server_name = "smtp.soverin.net"

[caldav]
base_url = "https://caldav.soverin.net"
username = "you@example.com"
password = "hunter2"
"#;

#[test]
fn parses_a_full_account_and_redacts_secrets() {
    let config: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
    assert_eq!(config.imap.addr, "imap.soverin.net:993");
    assert_eq!(config.imap.username, "you@example.com");
    assert_eq!(config.imap.password.as_ref().unwrap().expose(), "hunter2");

    let smtp = config.smtp.as_ref().expect("smtp present");
    assert_eq!(smtp.addr, "smtp.soverin.net:465");

    let caldav = config.caldav.as_ref().expect("caldav present");
    assert_eq!(caldav.base_url, "https://caldav.soverin.net");
    assert!(caldav.calendar.is_none());

    // Secrets never appear in Debug output (so logging a config is safe).
    let dump = format!("{config:?}");
    assert!(!dump.contains("hunter2"));
    assert_eq!(
        format!("{:?}", config.imap.password.as_ref().unwrap()),
        "Secret(<redacted>)"
    );

    // Builds the engine config without SMTP-absent branching surprises.
    let _ = config.imap_config(config.imap_password_credentials().unwrap());
}

#[test]
fn security_defaults_to_implicit_tls_and_parses_starttls() {
    // An account TOML with no `security` key connects exactly as before this field
    // existed: implicit TLS on both transports.
    let default_tls: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
    assert_eq!(default_tls.imap.security, ConnectionSecurity::ImplicitTls);
    assert_eq!(
        default_tls.smtp.as_ref().unwrap().security,
        ConnectionSecurity::ImplicitTls
    );

    // An explicit `security = "starttls"` on the IMAP-143 / submission-587 ports parses
    // and drives the engine's STARTTLS builders (exercised via `imap_config`).
    let starttls: AccountConfig = toml::from_str(
        "[imap]\naddr=\"mail.example.com:143\"\nserver_name=\"mail.example.com\"\n\
         username=\"u\"\npassword=\"p\"\nsecurity=\"starttls\"\n\
         [smtp]\naddr=\"mail.example.com:587\"\nserver_name=\"mail.example.com\"\n\
         security=\"starttls\"\n",
    )
    .expect("valid config");
    assert_eq!(starttls.imap.security, ConnectionSecurity::StartTls);
    assert_eq!(
        starttls.smtp.as_ref().unwrap().security,
        ConnectionSecurity::StartTls
    );
    let _ = starttls.imap_config(starttls.imap_password_credentials().unwrap());
}

#[test]
fn parses_an_imap_only_account() {
    let config: AccountConfig = toml::from_str(
        "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"p\"\n",
    )
    .expect("valid config");
    assert!(config.smtp.is_none() && config.caldav.is_none());
    let _ = config.imap_config(config.imap_password_credentials().unwrap());
}

#[test]
fn parses_an_explicit_caldav_calendar() {
    let config: AccountConfig = toml::from_str(
        "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"p\"\n\
         [caldav]\nbase_url=\"https://dav.example.com\"\nusername=\"u\"\npassword=\"p\"\n\
         calendar=\"work\"\n",
    )
    .expect("valid config");
    let caldav = config.caldav.as_ref().expect("caldav present");
    assert_eq!(caldav.calendar.as_deref(), Some("work"));
}

#[test]
fn replacing_a_password_preserves_every_endpoint_and_updates_caldav_too() {
    let original: AccountConfig = toml::from_str(
        "[imap]\naddr=\"mail.example.com:143\"\nserver_name=\"imap.example.com\"\n\
         username=\"alice@example.com\"\npassword=\"old\"\nsecurity=\"starttls\"\n\
         [smtp]\naddr=\"submit.example.com:587\"\nserver_name=\"smtp.example.com\"\n\
         security=\"starttls\"\n\
         [caldav]\nbase_url=\"https://dav.example.com/root\"\n\
         username=\"calendar-alias\"\npassword=\"old\"\ncalendar=\"work\"\n",
    )
    .expect("valid config");

    let updated = original
        .with_password("new\"secret\\value")
        .to_toml()
        .expect("serializable config");
    let parsed = load_str(&updated).expect("replacement config round-trips");

    assert_eq!(
        parsed.imap.password.as_ref().unwrap().expose(),
        "new\"secret\\value"
    );
    assert_eq!(
        parsed
            .caldav
            .as_ref()
            .unwrap()
            .password
            .as_ref()
            .unwrap()
            .expose(),
        "new\"secret\\value"
    );
    assert_eq!(parsed.imap.addr, "mail.example.com:143");
    assert_eq!(parsed.imap.server_name, "imap.example.com");
    assert_eq!(parsed.imap.security, ConnectionSecurity::StartTls);
    assert_eq!(parsed.smtp.as_ref().unwrap().addr, "submit.example.com:587");
    assert_eq!(parsed.caldav.as_ref().unwrap().username, "calendar-alias");
    assert_eq!(
        parsed.caldav.as_ref().unwrap().calendar.as_deref(),
        Some("work")
    );
}

fn config_with(username: &str, server_name: &str) -> AccountConfig {
    toml::from_str(&format!(
        "[imap]\naddr=\"{server_name}:993\"\nserver_name=\"{server_name}\"\n\
         username=\"{username}\"\npassword=\"p\"\n",
    ))
    .expect("valid config")
}

#[test]
fn account_id_is_case_insensitive_in_username_and_host() {
    // Case drift in the typed username (or host) must not mint a second identity for
    // the same mailbox: the id lowercases both.
    let a = config_with("Alice@Example.COM", "IMAP.Example.com")
        .account_id()
        .unwrap();
    let b = config_with("alice@example.com", "imap.example.com")
        .account_id()
        .unwrap();
    assert_eq!(a, b);
}

#[test]
fn account_id_differs_by_host_for_the_same_username() {
    // The same username on two different servers is two distinct accounts: the host
    // is part of the id, so they never collide in the shared engine store.
    let soverin = config_with("alice@example.com", "imap.soverin.net")
        .account_id()
        .unwrap();
    let fastmail = config_with("alice@example.com", "imap.fastmail.com")
        .account_id()
        .unwrap();
    assert_ne!(soverin, fastmail);
}

/// An account stored by the OAuth sign-in path: a grant, and no long-lived secret anywhere.
const OAUTH_SAMPLE: &str = r#"
[imap]
addr = "imap.example.net:993"
server_name = "imap.example.net"
username = "you@example.net"

[imap.oauth]
client_id = "client-abc"
refresh_token = "rt-value"
authorize_endpoint = "https://auth.example.net/authorize"
token_endpoint = "https://auth.example.net/token"
redirect_uri = "eu.allodia.mailcal://imap-oauth"
scopes = ["offline_access", "urn:ietf:params:oauth:scope:mail"]
issuer = "https://auth.example.net"

[smtp]
addr = "smtp.example.net:465"
server_name = "smtp.example.net"

[caldav]
base_url = "https://dav.example.net"
username = "you@example.net"
"#;

#[test]
fn an_oauth_account_parses_with_no_password_on_any_endpoint() {
    let config: AccountConfig = toml::from_str(OAUTH_SAMPLE).expect("valid config");
    assert!(config.is_oauth());
    assert!(config.imap.password.is_none());
    assert!(config.caldav.as_ref().unwrap().password.is_none());
    // The username survives: an OAuth account still names the mailbox its token was issued
    // for, which is what the SASL response carries as its `authzid`.
    assert_eq!(config.imap.username, "you@example.net");
    let grant = config.imap.oauth.as_ref().expect("grant");
    assert_eq!(grant.refresh_token.expose(), "rt-value");
    assert_eq!(grant.issuer.as_deref(), Some("https://auth.example.net"));
    // No password is stored, so there is no password credential to build.
    assert!(config.imap_password_credentials().is_none());
}

#[test]
fn an_oauth_account_round_trips_through_the_stored_form() {
    // Serialization is hand-written (the secrets are deliberately not `Serialize`), so this
    // is the only thing that notices a grant field silently no longer being written.
    let config: AccountConfig = toml::from_str(OAUTH_SAMPLE).expect("valid config");
    let parsed = load_str(&config.to_toml().expect("serializable")).expect("round-trips");
    assert!(parsed.is_oauth());
    assert!(parsed.imap.password.is_none());
    let grant = parsed.imap.oauth.as_ref().expect("grant");
    assert_eq!(grant.client_id, "client-abc");
    assert_eq!(grant.redirect_uri, "eu.allodia.mailcal://imap-oauth");
    assert_eq!(grant.scopes.len(), 2);
    assert_eq!(grant.issuer.as_deref(), Some("https://auth.example.net"));
    // Nothing in the stored form is readable as a secret in a debug dump.
    assert!(!format!("{parsed:?}").contains("rt-value"));
}

#[test]
fn replacing_the_password_of_an_oauth_account_does_nothing() {
    // The repair path for an OAuth account is a re-authorisation. Writing a password here
    // would leave an account carrying both credentials with nothing to say which is meant,
    // and the connect path would silently pick the grant while the user watched a password
    // they had just typed be ignored.
    let config: AccountConfig = toml::from_str(OAUTH_SAMPLE).expect("valid config");
    let updated = config.with_password("typed-by-hand");
    assert!(updated.imap.password.is_none());
    assert!(updated.is_oauth());
}

#[test]
fn granting_an_account_clears_the_password_it_used_to_have() {
    // A password account that signs in with OAuth instead: the old secret must go, on the
    // calendar endpoint as well as the mailbox, or it outlives the credential that replaced it.
    let config: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
    assert!(config.imap.password.is_some());
    let grant = toml::from_str::<AccountConfig>(OAUTH_SAMPLE)
        .expect("valid config")
        .imap
        .oauth
        .expect("grant");

    let updated = config.with_grant(grant);
    assert!(updated.is_oauth());
    assert!(updated.imap.password.is_none());
    assert!(updated.caldav.as_ref().unwrap().password.is_none());
}

#[test]
fn a_password_account_keeps_the_shape_it_had_before_oauth_existed() {
    // The migration guarantee: every account already on disk has no `[imap.oauth]` and no
    // `oauth` key is written for it, so its stored TOML is byte-for-byte what it was.
    let config: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
    let toml = config.to_toml().expect("serializable");
    assert!(!toml.contains("oauth"));
    assert!(!config.is_oauth());
}
