//! Tests for the discovery chain's **decisions**: the parts that decide where a credential
//! may be sent.
//!
//! These are deliberately offline and hit the pure functions directly rather than a mock
//! server. The security rules here (HTTPS-only, issuer match, S256 required) are exactly the
//! ones a plaintext `http://` mock endpoint would have to disable in order to run: a test
//! that turns off the check it is testing proves nothing. The HTTP plumbing around them
//! (`fetch_json`) is a thin GET-and-decode with no branching worth mocking TLS for, the live
//! round trip is covered by the on-device verification instead.

use super::*;

fn raw(issuer: &str, pkce: &[&str]) -> RawAuthServerMetadata {
    RawAuthServerMetadata {
        issuer: issuer.to_owned(),
        authorization_endpoint: Some("https://as.example.com/authorize".to_owned()),
        token_endpoint: Some("https://as.example.com/token".to_owned()),
        registration_endpoint: Some("https://as.example.com/register".to_owned()),
        revocation_endpoint: None,
        userinfo_endpoint: Some("https://as.example.com/userinfo".to_owned()),
        scopes_supported: vec!["offline_access".to_owned()],
        code_challenge_methods_supported: pkce.iter().map(|m| (*m).to_owned()).collect(),
        end_session_endpoint: None,
        prompt_values_supported: Vec::new(),
    }
}

#[test]
fn only_https_urls_are_followed() {
    assert!(require_https("https://api.example.com/x").is_ok());
    // A plaintext hop would put the authorization code (and the token it mints) in the
    // clear. Refused, not warned about.
    assert!(matches!(
        require_https("http://api.example.com/x"),
        Err(DiscoveryError::InsecureUrl(_))
    ));
    // Non-HTTP schemes are equally unacceptable as a discovery hop.
    assert!(require_https("file:///etc/passwd").is_err());
    assert!(require_https("not a url").is_err());
}

#[test]
fn the_well_known_path_is_inserted_before_the_issuer_path_not_appended() {
    // RFC 8414 §3.1. Getting this backwards silently 404s on every tenant-scoped issuer and
    // looks like "the server doesn't support discovery".
    let issuer = url::Url::parse("https://example.com/tenant-a").unwrap();
    assert_eq!(
        well_known_url(&issuer, AS_WELL_KNOWN),
        "https://example.com/.well-known/oauth-authorization-server/tenant-a"
    );
    // A path-less issuer just takes the prefix (and no trailing slash is left behind).
    let bare = url::Url::parse("https://example.com/").unwrap();
    assert_eq!(
        well_known_url(&bare, AS_WELL_KNOWN),
        "https://example.com/.well-known/oauth-authorization-server"
    );
}

#[test]
fn the_resource_metadata_pointer_is_read_out_of_the_challenge() {
    // The shape api.fastmail.com actually sends.
    assert_eq!(
        resource_metadata_url(
            r#"Bearer resource_metadata="https://api.fastmail.com/.well-known/oauth-protected-resource""#
        )
        .as_deref(),
        Some("https://api.fastmail.com/.well-known/oauth-protected-resource")
    );
    // Other params before it, and another challenge after it, must not confuse the read.
    assert_eq!(
        resource_metadata_url(
            r#"Basic realm="jmap", Bearer realm="x", resource_metadata="https://as.example.com/meta", error="invalid_token""#
        )
        .as_deref(),
        Some("https://as.example.com/meta")
    );
    // A bare (unquoted) value is legal token68-ish syntax in the wild.
    assert_eq!(
        resource_metadata_url("Bearer resource_metadata=https://as.example.com/meta").as_deref(),
        Some("https://as.example.com/meta")
    );
    // A challenge with no pointer falls through to the well-known default, not a wrong URL.
    assert!(resource_metadata_url(r#"Basic realm="jmap""#).is_none());
    assert!(resource_metadata_url("Bearer").is_none());
}

#[test]
fn metadata_claiming_a_different_issuer_is_rejected() {
    // RFC 8414 §3.3, and the reason it exists: a compromised resource that points us at an
    // attacker-controlled issuer must not be able to hand us its token endpoint.
    let err = validate(
        raw("https://evil.example.net", &["S256"]),
        "https://as.example.com",
        "https://as.example.com/.well-known/oauth-authorization-server",
    )
    .unwrap_err();
    assert!(matches!(err, DiscoveryError::IssuerMismatch { .. }));
}

#[test]
fn a_trailing_slash_does_not_count_as_an_issuer_mismatch() {
    // Trailing slashes are not significant in an issuer identifier; treating them as a
    // mismatch would break real servers for no security gain.
    assert!(
        validate(
            raw("https://as.example.com/", &["S256"]),
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        )
        .is_ok()
    );
}

#[test]
fn a_server_without_s256_pkce_is_declined() {
    // Without S256 a public client's code exchange is unprotected. We fall back to the manual
    // secret rather than run a weaker flow the user cannot see.
    let err = validate(
        raw("https://as.example.com", &["plain"]),
        "https://as.example.com",
        "https://as.example.com/.well-known/oauth-authorization-server",
    )
    .unwrap_err();
    assert!(matches!(err, DiscoveryError::NoPkce(_)));

    // …and advertising none at all is the same answer.
    assert!(matches!(
        validate(
            raw("https://as.example.com", &[]),
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        ),
        Err(DiscoveryError::NoPkce(_))
    ));
}

#[test]
fn endpoints_named_over_plaintext_are_rejected_even_when_the_issuer_matches() {
    // A valid, matching issuer that names an http:// token endpoint is still refused; the
    // per-URL check is what actually guards the wire, not the issuer comparison.
    let mut doc = raw("https://as.example.com", &["S256"]);
    doc.token_endpoint = Some("http://as.example.com/token".to_owned());
    assert!(matches!(
        validate(
            doc,
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        ),
        Err(DiscoveryError::InsecureUrl(_))
    ));
}

#[test]
fn metadata_missing_a_required_endpoint_is_malformed() {
    let mut doc = raw("https://as.example.com", &["S256"]);
    doc.authorization_endpoint = None;
    assert!(matches!(
        validate(
            doc,
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        ),
        Err(DiscoveryError::MalformedMetadata { .. })
    ));
}

#[test]
fn a_valid_document_yields_the_endpoints_and_optional_registration() {
    let metadata = validate(
        raw("https://as.example.com", &["S256", "plain"]),
        "https://as.example.com",
        "https://as.example.com/.well-known/oauth-authorization-server",
    )
    .unwrap();
    assert_eq!(
        metadata.authorization_endpoint,
        "https://as.example.com/authorize"
    );
    assert_eq!(metadata.token_endpoint, "https://as.example.com/token");
    assert_eq!(
        metadata.registration_endpoint.as_deref(),
        Some("https://as.example.com/register")
    );
    assert!(metadata.revocation_endpoint.is_none());
    assert_eq!(
        metadata.userinfo_endpoint.as_deref(),
        Some("https://as.example.com/userinfo")
    );
}

#[test]
fn a_document_naming_no_logout_or_prompt_support_yields_neither() {
    // The absent case is the one that matters: a caller reads these to decide whether to send a
    // `prompt` at all and whether a sign-out has anywhere to go, and inventing either would put an
    // unadvertised parameter on a request a server is free to refuse outright.
    let metadata = validate(
        raw("https://as.example.com", &["S256"]),
        "https://as.example.com",
        "https://as.example.com/.well-known/oauth-authorization-server",
    )
    .unwrap();
    assert!(metadata.end_session_endpoint.is_none());
    assert!(metadata.prompt_values_supported.is_empty());
}

#[test]
fn a_logout_endpoint_and_the_prompt_values_are_read_when_advertised() {
    let mut document = raw("https://as.example.com", &["S256"]);
    document.end_session_endpoint = Some("https://as.example.com/end-session".to_owned());
    document.prompt_values_supported = ["login", "create"]
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    let metadata = validate(
        document,
        "https://as.example.com",
        "https://as.example.com/.well-known/oauth-authorization-server",
    )
    .unwrap();
    assert_eq!(
        metadata.end_session_endpoint.as_deref(),
        Some("https://as.example.com/end-session")
    );
    assert!(
        metadata
            .prompt_values_supported
            .iter()
            .any(|value| value == "create")
    );
}

#[test]
fn a_logout_endpoint_named_over_plaintext_is_rejected() {
    // It is opened in the person's browser carrying an id_token_hint, which names them.
    let mut document = raw("https://as.example.com", &["S256"]);
    document.end_session_endpoint = Some("http://as.example.com/end-session".to_owned());
    assert!(matches!(
        validate(
            document,
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        ),
        Err(DiscoveryError::InsecureUrl(_))
    ));
}

#[test]
fn a_userinfo_endpoint_is_held_to_the_same_tls_rule_as_every_other() {
    // It carries an access token like the token endpoint does, so it is not the "merely
    // informational" URL its name suggests -- a plaintext one would put a live bearer on the wire.
    let mut document = raw("https://as.example.com", &["S256"]);
    document.userinfo_endpoint = Some("http://as.example.com/userinfo".to_owned());
    assert!(matches!(
        validate(
            document,
            "https://as.example.com",
            "https://as.example.com/.well-known/oauth-authorization-server",
        ),
        Err(DiscoveryError::InsecureUrl(_))
    ));
}

// --- The TLS rule and its one exemption
// ------------------------------------------------------------

#[test]
fn a_plaintext_hop_to_a_real_host_is_refused() {
    // The rule the exemption below must not weaken: discovery decides where a credential is sent,
    // so a plaintext hop to anywhere reachable is refused outright rather than warned about.
    for url in [
        "http://example.com/.well-known/oauth-authorization-server",
        "http://192.168.1.10:3000/",
        "ftp://example.com/",
    ] {
        assert!(
            super::require_https(url).is_err(),
            "{url} should not have been accepted"
        );
    }
}

#[test]
fn loopback_over_plaintext_is_allowed_because_there_is_no_hop() {
    // An authorization server on the developer's own machine cannot present a certificate for a
    // name it does not own, and a request that never reaches a network has nothing to protect.
    for url in [
        "http://127.0.0.1:3000/",
        "http://localhost:3000/",
        "http://LOCALHOST:3000/",
        "http://[::1]:3000/",
    ] {
        assert!(
            super::require_https(url).is_ok(),
            "{url} should have been accepted"
        );
    }
}

#[test]
fn a_host_that_merely_looks_like_loopback_is_not_loopback() {
    // Which is why the check is on the parsed host and not on the string: both of these carry a
    // loopback-looking substring and neither is this machine.
    for url in [
        "http://127.0.0.1.example.com/",
        "http://localhost.example.com/",
        "http://evil.example/?redirect=localhost",
    ] {
        assert!(
            super::require_https(url).is_err(),
            "{url} should not have been accepted"
        );
    }
}
