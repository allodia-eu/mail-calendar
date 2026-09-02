//! Tests for what survives the browser hop, and what the completed sign-in stores.
//!
//! The network halves are not mocked: a fake authorization server answers whatever it is
//! asked and would prove only that the code calls itself. What is worth pinning is the
//! handle, because every field in it is one a server checks later and somewhere else: a
//! dropped `redirect_uri` is rejected on the next refresh, a dropped `issuer` silently stops
//! checking RFC 9207, and a dropped STARTTLS flag connects an account to a port it does not
//! use.

use super::*;

fn pending() -> PendingImapLogin {
    PendingImapLogin {
        email: "alice@example.com".to_owned(),
        imap_host: "imap.example.com".to_owned(),
        smtp_host: Some("smtp.example.com".to_owned()),
        caldav_base_url: Some("https://dav.example.com".to_owned()),
        imap_starttls: true,
        smtp_starttls: true,
        client_id: "client-abc".to_owned(),
        client_secret: None,
        authorize_endpoint: "https://login.example.com/authorize".to_owned(),
        token_endpoint: "https://login.example.com/token".to_owned(),
        redirect_uri: "eu.allodia.mailcal://imap-oauth".to_owned(),
        scopes: vec![
            "offline_access".to_owned(),
            "urn:ietf:params:oauth:scope:mail".to_owned(),
        ],
        issuer: Some("https://login.example.com".to_owned()),
        state: "state-xyz".to_owned(),
        verifier: "verifier".to_owned(),
    }
}

#[test]
fn the_pending_handle_round_trips_through_the_host() {
    // The host carries this opaque across a browser hop, so it must survive serialization
    // exactly: a dropped verifier or state fails the exchange with a message that points
    // nowhere near the cause.
    let encoded = serde_json::to_string(&pending()).unwrap();
    let decoded: PendingImapLogin = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.state, "state-xyz");
    assert_eq!(decoded.verifier, "verifier");
    assert_eq!(decoded.redirect_uri, "eu.allodia.mailcal://imap-oauth");
    assert!(decoded.imap_starttls && decoded.smtp_starttls);
}

#[test]
fn the_completed_account_stores_the_grant_and_no_secret_anywhere() {
    let (setup, _grant) = pending().into_grant("rt-value".to_owned());
    let toml = mailcal_account::build_config_toml(&setup).expect("serializable");
    let config = mailcal_account::load_str(&toml).expect("round-trips");

    assert!(config.is_oauth());
    assert!(config.imap.password.is_none());
    // The calendar rides on the same grant: writing a password there would leave a stored
    // secret nothing ever presents.
    assert!(config.caldav.as_ref().unwrap().password.is_none());
    let grant = config.imap.oauth.as_ref().expect("grant");
    assert_eq!(grant.client_id, "client-abc");
    assert_eq!(grant.refresh_token.expose(), "rt-value");
    assert_eq!(grant.redirect_uri, "eu.allodia.mailcal://imap-oauth");
    assert_eq!(grant.issuer.as_deref(), Some("https://login.example.com"));
    assert!(!format!("{config:?}").contains("rt-value"));
}

#[test]
fn the_detected_transports_survive_into_the_stored_account() {
    // A STARTTLS account signed in over the browser must still dial 143/587. The security
    // travels as a flag across the hop and is the kind of thing a refactor drops silently:
    // the account then connects to a port the provider may not even have open.
    let (setup, _grant) = pending().into_grant("rt".to_owned());
    let config = mailcal_account::load_str(&mailcal_account::build_config_toml(&setup).unwrap())
        .expect("round-trips");
    assert_eq!(config.imap.addr, "imap.example.com:143");
    assert_eq!(
        config.imap.security,
        mailcal_account::ConnectionSecurity::StartTls
    );
    let smtp = config.smtp.as_ref().expect("smtp");
    assert_eq!(smtp.addr, "smtp.example.com:587");
    assert_eq!(smtp.security, mailcal_account::ConnectionSecurity::StartTls);
}

#[test]
fn an_imap_grant_names_no_rfc_8707_resource() {
    // There is no URI form for an IMAP endpoint in the profile, and a server that scopes
    // tokens by resource applies its default without one. Inventing `imap://…` would risk
    // `invalid_target` on the exchange and on every refresh after it.
    let (_setup, grant) = pending().into_grant("rt".to_owned());
    assert!(grant.resource.is_none());
    assert!(grant.provider_config().resource.is_none());
}

#[test]
fn the_authorization_request_targets_the_address_and_carries_pkce() {
    let start = start_login(pending()).expect("an authorization request");
    assert!(
        start
            .authorization_url
            .starts_with("https://login.example.com/authorize?"),
        "{}",
        start.authorization_url
    );
    assert!(start.authorization_url.contains("client_id=client-abc"));
    assert!(
        start
            .authorization_url
            .contains("code_challenge_method=S256")
    );
    // The address is known, so the provider targets it rather than showing a picker.
    assert!(
        start
            .authorization_url
            .contains("login_hint=alice%40example.com")
    );

    // …and the state and verifier it minted are what the completion will be checked against.
    let decoded: PendingImapLogin = serde_json::from_str(&start.pending).unwrap();
    assert!(!decoded.state.is_empty());
    assert!(!decoded.verifier.is_empty());
    assert!(start.authorization_url.contains(&decoded.state));
}

#[test]
fn the_three_offers_survive_the_ffi_boundary_distinctly() {
    // The middle case is the reason the enum has three arms rather than a flag: collapsing
    // "the provider admits only pre-registered applications" into "no OAuth here" leaves the
    // user with a password form and no idea why.
    assert_eq!(
        ImapAuthOffer::from(ImapAuth::SignIn {
            issuer: "https://login.example.com".to_owned(),
            provider_label: Some("Example Mail".to_owned()),
            password_also_works: true,
        }),
        ImapAuthOffer::SignIn {
            issuer: "https://login.example.com".to_owned(),
            provider_label: Some("Example Mail".to_owned()),
            password_also_works: true,
        }
    );
    assert_eq!(
        ImapAuthOffer::from(ImapAuth::RegistrationNeeded {
            password_also_works: true
        }),
        ImapAuthOffer::RegistrationNeeded {
            password_also_works: true
        }
    );
    assert_eq!(
        ImapAuthOffer::from(ImapAuth::Password),
        ImapAuthOffer::Password
    );
}
