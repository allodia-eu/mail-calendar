//! Gated live test of the OAuth discovery chain against a **real** JMAP server.
//!
//! The offline unit tests cover every decision the chain makes (HTTPS-only, issuer match, S256,
//! scope selection). What they cannot cover is what a real server's HTTP actually looks like;
//! and that is where this feature's first bug was: `api.fastmail.com/.well-known/jmap` answers
//! **302** to `/jmap/session`, and only *that* URL returns the `401` whose challenge names the
//! protected-resource metadata (at a path-scoped URL, not the origin default). A test that only
//! mocked a 401 would have been green over it.
//!
//! Skips unless `MAILCAL_LIVE_JMAP_ORIGIN` is set, so the offline suite stays green with no
//! network: the same discipline as `mailcal-account`'s `live_jmap.rs`. Run it with:
//!
//! ```text
//! MAILCAL_LIVE_JMAP_ORIGIN=https://api.fastmail.com cargo test -p mailcal-oauth --test live_discovery -- --nocapture
//! ```
//!
//! It deliberately stops **short of registration**: it must never create a client on someone's
//! account as a side effect of running the test suite.

use mailcal_oauth::{
    discover_auth_server, discover_protected_resource, discovery_client, grants_mail_access,
    select_scopes,
};

/// The JMAP session path the resource lives behind (RFC 8620 §2).
const SESSION_PATH: &str = "/.well-known/jmap";

fn origin() -> Option<String> {
    std::env::var("MAILCAL_LIVE_JMAP_ORIGIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[tokio::test]
async fn a_real_server_advertises_a_usable_authorization_server() {
    let Some(origin) = origin() else {
        eprintln!("skipping: set MAILCAL_LIVE_JMAP_ORIGIN to run");
        return;
    };
    // The shared TLS policy: a bare reqwest client has no crypto provider in this
    // workspace and panics on first use.
    let http = discovery_client().expect("discovery http client builds");
    let session_url = format!("{}{SESSION_PATH}", origin.trim_end_matches('/'));

    let protected = discover_protected_resource(&http, &session_url)
        .await
        .unwrap_or_else(|err| panic!("resource discovery failed for {session_url}: {err}"));
    println!("issuer:   {}", protected.issuer);
    println!("resource: {:?}", protected.resource);

    let metadata = discover_auth_server(&http, &protected.issuer)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "authorization-server discovery failed for {}: {err}",
                protected.issuer
            )
        });
    println!("authorize: {}", metadata.authorization_endpoint);
    println!("token:     {}", metadata.token_endpoint);
    println!("register:  {:?}", metadata.registration_endpoint);

    // A server we would actually offer sign-in for must support dynamic registration (we have no
    // pre-registered client) and grant mail access.
    let scopes = select_scopes(&metadata);
    println!("scopes:    {scopes:?}");
    assert!(
        metadata.registration_endpoint.is_some(),
        "no registration endpoint; sign-in could not be offered for this server",
    );
    assert!(
        grants_mail_access(&metadata, &scopes),
        "the selected scopes grant no mail access: {scopes:?}",
    );
    // RFC 8707: a server that publishes a canonical resource URI generally *requires* it as the
    // token request's `resource`. Fastmail answers `invalid_target` without it: the bug this
    // assertion exists to stop coming back.
    if let Some(resource) = &protected.resource {
        assert!(
            resource.starts_with("https://"),
            "the resource indicator must be an https URI: {resource}",
        );
    }
}
