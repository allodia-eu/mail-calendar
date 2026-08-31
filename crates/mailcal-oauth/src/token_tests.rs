//! The token exchanges, against a mock endpoint that records what was actually posted.
use std::io::{Read, Write};

use super::*;
use crate::provider::OAuthProviderConfig;

/// A blocking single-shot mock token endpoint: serves `response` to one request,
/// so the live reqwest path runs offline (mirrors the engine's provider tests).
fn mock_endpoint(response: String) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}/token")
}

/// Like [`mock_endpoint`], but also hands back the **raw request bytes** the client sent, so
/// a test can assert on the urlencoded form body (e.g. whether `client_secret` is present).
fn mock_endpoint_capturing(response: String) -> (String, std::sync::mpsc::Receiver<String>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let _ = tx.send(String::from_utf8_lossy(&buf[..n]).into_owned());
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}/token"), rx)
}

fn http_response(status_line: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn provider_at(token_endpoint: String) -> OAuthProviderConfig {
    OAuthProviderConfig {
        authorize_endpoint: "https://example/authorize".to_owned(),
        token_endpoint,
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://oauth".to_owned(),
        scopes: vec!["offline_access".to_owned(), "openid".to_owned()],
        resource: None,
        style: crate::provider::AuthStyle::Microsoft,
    }
}

fn epoch() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
}

#[tokio::test]
async fn exchange_code_parses_tokens_and_resolves_absolute_expiry() {
    let body = r#"{"token_type":"Bearer","scope":"Mail.Read","expires_in":3600,"access_token":"AT","refresh_token":"RT"}"#;
    let provider = provider_at(mock_endpoint(http_response("200 OK", body)));
    let tokens = exchange_code(
        &reqwest::Client::new(),
        &provider,
        "code123",
        "verifier",
        epoch(),
    )
    .await
    .unwrap();

    assert_eq!(tokens.access_token.expose(), "AT");
    assert_eq!(tokens.refresh_token.as_ref().unwrap().expose(), "RT");
    // expires_at = now + expires_in (3600s after the epoch).
    assert_eq!(tokens.expires_at, epoch() + Duration::seconds(3600));
    assert_eq!(tokens.scope, "Mail.Read");
}

#[tokio::test]
async fn a_google_desktop_secret_is_sent_on_both_the_code_exchange_and_the_refresh() {
    // A Google Desktop client's non-confidential secret must ride on BOTH grants; Google's
    // token endpoint rejects the PKCE exchange without it, and a fix that missed the refresh
    // would break the account ~1h after setup.
    let ok =
        r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT"}"#;

    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let mut provider = provider_at(endpoint);
    provider.client_secret = Some("GOCSPX-desktop".to_owned());
    exchange_code(
        &reqwest::Client::new(),
        &provider,
        "code",
        "verifier",
        epoch(),
    )
    .await
    .unwrap();
    let exchange_req = rx.recv().unwrap();
    assert!(
        exchange_req.contains("client_secret=GOCSPX-desktop"),
        "code exchange must carry the Desktop secret: {exchange_req}"
    );

    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let mut provider = provider_at(endpoint);
    provider.client_secret = Some("GOCSPX-desktop".to_owned());
    refresh(&reqwest::Client::new(), &provider, "old-RT", epoch())
        .await
        .unwrap();
    let refresh_req = rx.recv().unwrap();
    assert!(
        refresh_req.contains("client_secret=GOCSPX-desktop"),
        "refresh must carry the Desktop secret too: {refresh_req}"
    );
}

#[tokio::test]
async fn the_rfc8707_resource_rides_on_both_the_exchange_and_the_refresh() {
    // A server that issues resource-scoped tokens rejects a request that does not name its
    // target; Fastmail answers `invalid_target`, which is exactly how this surfaced on a real
    // device. Sending it on the exchange but NOT the refresh is the subtle version of the same
    // bug: setup succeeds and the account dies about an hour later.
    let ok =
        r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT"}"#;
    let resource = "https://api.example.com/jmap/session";

    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let mut provider = provider_at(endpoint);
    provider.resource = Some(resource.to_owned());
    exchange_code(&reqwest::Client::new(), &provider, "code", "v", epoch())
        .await
        .unwrap();
    let exchange = rx.recv().unwrap();
    assert!(
        exchange.contains("resource=https%3A%2F%2Fapi.example.com%2Fjmap%2Fsession"),
        "the code exchange must name the resource: {exchange}"
    );

    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let mut provider = provider_at(endpoint);
    provider.resource = Some(resource.to_owned());
    refresh(&reqwest::Client::new(), &provider, "old-RT", epoch())
        .await
        .unwrap();
    let refreshed = rx.recv().unwrap();
    assert!(
        refreshed.contains("resource=https%3A%2F%2Fapi.example.com%2Fjmap%2Fsession"),
        "the refresh must name the resource too: {refreshed}"
    );
}

#[tokio::test]
async fn no_resource_is_sent_when_the_server_named_none() {
    // The integrated providers (Microsoft, Google) scope by scope alone. Sending an empty or
    // spurious `resource` to them is at best noise and at worst a rejected grant.
    let ok = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT"}"#;
    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let provider = provider_at(endpoint); // resource: None
    exchange_code(&reqwest::Client::new(), &provider, "code", "v", epoch())
        .await
        .unwrap();
    let request = rx.recv().unwrap();
    assert!(
        !request.contains("resource="),
        "no resource must be sent when none was discovered: {request}"
    );
}

#[tokio::test]
async fn a_public_client_sends_no_client_secret() {
    // The regression guard for the common case: a true public client (client_secret: None)
    // must never put `client_secret` on the wire.
    let ok =
        r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT"}"#;
    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let provider = provider_at(endpoint); // client_secret: None
    exchange_code(
        &reqwest::Client::new(),
        &provider,
        "code",
        "verifier",
        epoch(),
    )
    .await
    .unwrap();
    let request = rx.recv().unwrap();
    assert!(
        !request.contains("client_secret"),
        "a public client must send no client_secret: {request}"
    );
}

/// The live bug this whole classification exists for.
///
/// A refresh used to send `provider.scopes`: this build's CURRENT list. RFC 6749 §6 requires
/// the requested scope to be a subset of what was granted, so the first time the list grew,
/// every grant issued before it was refused with `invalid_scope` and the account stopped
/// working entirely: not the new feature, the whole account. An omitted scope means "the same
/// as originally granted", which is the only value that cannot go stale.
#[tokio::test]
async fn a_refresh_names_no_scope_so_a_grant_predating_a_new_one_still_works() {
    let ok =
        r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"RT"}"#;
    let (endpoint, rx) = mock_endpoint_capturing(http_response("200 OK", ok));
    let mut provider = provider_at(endpoint);
    // A build that has since grown a scope the stored grant was never issued for.
    provider.scopes = vec![
        "offline_access".to_owned(),
        "openid".to_owned(),
        "mailcal:accounts:read".to_owned(),
    ];
    refresh(&reqwest::Client::new(), &provider, "old-RT", epoch())
        .await
        .unwrap();
    let request = rx.recv().unwrap();
    assert!(
        !request.contains("scope="),
        "a refresh must name no scope at all, or a grant predating one of them is refused \
         with invalid_scope and the account dies: {request}"
    );
}

/// The other half of the rule, and the reason dropping it from the refresh is safe: asking
/// for a scope happens on the AUTHORISATION request, which is where consent is given. That is
/// what widens a grant, and it is what signing in again re-runs: so it must carry the full
/// current list however old the grant being replaced was.
#[test]
fn the_authorization_request_still_asks_for_every_scope_this_build_wants() {
    let mut provider = provider_at("https://example/token".to_owned());
    provider.scopes = vec!["openid".to_owned(), "mailcal:accounts:read".to_owned()];
    let url = provider.authorization_url("state", "challenge", None);
    assert!(
        url.contains("scope=openid+mailcal%3Aaccounts%3Aread")
            || url.contains("scope=openid%20mailcal%3Aaccounts%3Aread"),
        "consent is asked for here, so this names every scope: {url}"
    );
}

#[tokio::test]
async fn an_invalid_scope_refusal_is_classified_as_under_scoped_not_as_a_dead_grant() {
    // The two have the same remedy (sign in again) and very different meanings: a dead grant
    // means signed out, an under-scoped one means still signed in with a feature asleep. A
    // caller that flattened them would sign somebody out over a scope.
    let body = r#"{"error":"invalid_scope","error_description":"unable to issue scope mailcal:accounts:read"}"#;
    let provider = provider_at(mock_endpoint(http_response("400 Bad Request", body)));
    let err = refresh(&reqwest::Client::new(), &provider, "old-RT", epoch())
        .await
        .unwrap_err();
    assert_eq!(err.refusal(), crate::GrantRefusal::Underscoped);
    assert!(err.refusal().needs_reauth());
    assert!(
        !err.is_invalid_grant(),
        "an under-scoped grant is not a dead one"
    );
}

#[tokio::test]
async fn refresh_without_a_new_refresh_token_yields_none_so_the_caller_keeps_the_old() {
    // Microsoft often omits refresh_token on refresh (the old one stays valid).
    let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT2"}"#;
    let provider = provider_at(mock_endpoint(http_response("200 OK", body)));
    let tokens = refresh(&reqwest::Client::new(), &provider, "old-RT", epoch())
        .await
        .unwrap();
    assert_eq!(tokens.access_token.expose(), "AT2");
    assert!(tokens.refresh_token.is_none());
}

#[tokio::test]
async fn an_oauth_error_body_becomes_a_classified_endpoint_error() {
    let body = r#"{"error":"invalid_grant","error_description":"AADSTS70008: expired"}"#;
    let provider = provider_at(mock_endpoint(http_response("400 Bad Request", body)));
    let err = exchange_code(&reqwest::Client::new(), &provider, "stale", "v", epoch())
        .await
        .unwrap_err();
    match err {
        OAuthError::Endpoint { error, description } => {
            assert_eq!(error, "invalid_grant");
            assert!(description.unwrap().contains("expired"));
        }
        other => panic!("expected Endpoint, got {other:?}"),
    }
}

#[tokio::test]
async fn a_non_json_error_body_still_surfaces_the_status() {
    let provider = provider_at(mock_endpoint(http_response(
        "500 Server Error",
        "upstream boom",
    )));
    let err = refresh(&reqwest::Client::new(), &provider, "rt", epoch())
        .await
        .unwrap_err();
    match err {
        OAuthError::Endpoint { error, description } => {
            assert!(error.contains("500"));
            assert_eq!(description.as_deref(), Some("upstream boom"));
        }
        other => panic!("expected Endpoint, got {other:?}"),
    }
}

#[test]
fn is_expired_respects_the_skew_margin() {
    let tokens = TokenSet {
        access_token: Secret::new("AT".into()),
        refresh_token: None,
        expires_at: epoch() + Duration::seconds(3600),
        scope: String::new(),
        token_type: "Bearer".to_owned(),
    };
    // Fresh at the epoch with a 5-minute skew.
    assert!(!tokens.is_expired(epoch(), Duration::minutes(5)));
    // 3595s in, the 5-minute skew pushes past expiry → refresh.
    assert!(tokens.is_expired(epoch() + Duration::seconds(3595), Duration::minutes(5)));
    // Exactly at expiry is expired.
    assert!(tokens.is_expired(epoch() + Duration::seconds(3600), Duration::ZERO));
}

#[test]
fn token_debug_never_leaks_secrets() {
    let tokens = TokenSet {
        access_token: Secret::new("super-secret-access".into()),
        refresh_token: Some(Secret::new("super-secret-refresh".into())),
        expires_at: epoch(),
        scope: String::new(),
        token_type: "Bearer".to_owned(),
    };
    let dump = format!("{tokens:?}");
    assert!(!dump.contains("super-secret-access"));
    assert!(!dump.contains("super-secret-refresh"));
}
