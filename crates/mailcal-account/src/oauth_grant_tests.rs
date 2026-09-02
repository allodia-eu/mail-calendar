//! Tests for [`OAuthGrant`]: what must survive persistence, and what must never appear in a
//! log.
//!
//! Every assertion here is about a field that is silently optional at the type level and
//! fatal at runtime if dropped. A grant that loses `resource` starts failing `invalid_target`
//! an hour after setup; one that loses `redirect_uri` is rejected on the next refresh; one
//! that loses `issuer` quietly stops checking RFC 9207 on re-authorisation.

use super::*;

fn grant() -> OAuthGrant {
    OAuthGrant {
        client_id: "03be41ae".to_owned(),
        client_secret: None,
        refresh_token: Secret::new("rt-secret".to_owned()),
        authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
        token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
        redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
        scopes: vec![
            "offline_access".to_owned(),
            "urn:ietf:params:oauth:scope:mail".to_owned(),
        ],
        resource: Some("https://api.example.com/jmap/session".to_owned()),
        issuer: Some("https://api.example.com".to_owned()),
    }
}

#[test]
fn the_provider_config_uses_the_discovered_endpoints_and_no_vendor_extensions() {
    let config = grant().provider_config();
    assert_eq!(
        config.authorize_endpoint,
        "https://api.example.com/oauth/authorize"
    );
    assert_eq!(
        config.token_endpoint,
        "https://api.example.com/oauth/refresh"
    );
    assert_eq!(config.style, AuthStyle::Discovered);
    assert!(config.client_secret.is_none());
    // The RFC 8707 target must survive into the provider config, or every refresh this grant
    // ever makes is rejected with `invalid_target`.
    assert_eq!(
        config.resource.as_deref(),
        Some("https://api.example.com/jmap/session")
    );
    // …and the RFC 9207 issuer, or a re-authorisation stops checking which server answered.
    assert_eq!(
        config.expected_issuer.as_deref(),
        Some("https://api.example.com")
    );

    // A Discovered authorization URL carries the RFC params and nothing else; no
    // `prompt`, no `access_type`, no `response_mode`. Guessing at a vendor extension a
    // discovered server never documented is how a working flow starts 400ing.
    let url = config.authorization_url("state", "challenge", None);
    assert!(url.starts_with("https://api.example.com/oauth/authorize?"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("response_type=code"));
    assert!(!url.contains("prompt="));
    assert!(!url.contains("access_type="));
    assert!(!url.contains("response_mode="));
}

#[test]
fn a_login_hint_is_passed_through_when_the_address_is_known() {
    let config = grant().provider_config();
    let url = config.authorization_url("s", "c", Some("alice@example.com"));
    assert!(url.contains("login_hint=alice%40example.com"));
    // …and a blank one is dropped rather than sent empty.
    assert!(
        !config
            .authorization_url("s", "c", Some("  "))
            .contains("login_hint")
    );
}

#[test]
fn debug_never_leaks_either_secret() {
    let mut with_secret = grant();
    with_secret.client_secret = Some(Secret::new("cs-secret".to_owned()));
    let dump = format!("{with_secret:?}");
    assert!(!dump.contains("rt-secret"));
    assert!(!dump.contains("cs-secret"));
}

#[test]
fn the_table_round_trips_every_field_the_next_refresh_needs() {
    // Serialization is written by hand (the secrets are deliberately not `Serialize`), so
    // nothing but this test notices a field that stops being written.
    let table = grant().to_table();
    let text = toml::to_string(&table).unwrap();
    let read: OAuthGrant = toml::from_str(&text).unwrap();
    assert_eq!(read.client_id, "03be41ae");
    assert_eq!(read.refresh_token.expose(), "rt-secret");
    assert_eq!(read.redirect_uri, "eu.allodia.mailcal://jmap-oauth");
    assert_eq!(read.scopes.len(), 2);
    assert_eq!(
        read.resource.as_deref(),
        Some("https://api.example.com/jmap/session")
    );
    assert_eq!(read.issuer.as_deref(), Some("https://api.example.com"));
}

#[test]
fn a_grant_stored_before_the_issuer_field_existed_still_loads() {
    // Backward compatibility, and the reason `issuer` is `Option` rather than defaulted to
    // the token endpoint's host: an account signed in before RFC 9207 checking existed has
    // nothing to compare against, and inventing a value would reject its next re-consent.
    let older = "client_id = \"c\"\nrefresh_token = \"rt\"\n\
                 authorize_endpoint = \"https://api.example.com/authorize\"\n\
                 token_endpoint = \"https://api.example.com/token\"\n\
                 redirect_uri = \"eu.allodia.mailcal://jmap-oauth\"\n\
                 scopes = [\"offline_access\"]\n";
    let read: OAuthGrant = toml::from_str(older).unwrap();
    assert!(read.issuer.is_none());
    assert!(read.resource.is_none());
    assert!(read.provider_config().expected_issuer.is_none());
}
