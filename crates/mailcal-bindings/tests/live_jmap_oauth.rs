//! Gated live check of "Sign in with your provider" for a JMAP account, through the public FFI a
//! client calls: the pre-flight, `begin_jmap_login`, `complete_jmap_login` and `add_account`,
//! against the harness's sign-in server (`stalwart-oauth` in `docker/stalwart/docker-compose.yml`).
//!
//! What runs for real: RFC 9728 discovery from the session URL, RFC 8414 metadata, RFC 7591
//! registration, the PKCE authorization request with its RFC 8707 resource indicator, the code
//! exchange, and an account connected on the resulting grant. The one step a test cannot take the
//! way a user does is the login page, so `sign_in_as_the_user` (in `live_oauth/`) posts the page's
//! own form to Stalwart instead.
//!
//! Skips unless `STALWART_OAUTH_HTTP_ADDR` is set, so the offline `cargo test` stays green. Run
//! locally:
//! ```sh
//! (cd docker/stalwart && docker compose up -d --wait)
//! STALWART_OAUTH_HTTP_ADDR=localhost:28081 STALWART_HTTP_ADDR=127.0.0.1:28080 \
//!   cargo test -p mailcal-bindings --test live_jmap_oauth -- --nocapture
//! ```

mod live_oauth;

use live_oauth::{LOGIN as EMAIL, PASSWORD, RecordingStore, app, param, sign_in_as_the_user};

/// A custom-scheme redirect of the shape every client registers (`<app id>://jmap-oauth`). Nothing
/// dereferences it: the code travels back in the callback URL this test builds.
const REDIRECT_URI: &str = "mailcal.test://jmap-oauth";

#[test]
fn a_jmap_account_signs_in_through_discovery_and_registration() {
    let Ok(addr) = std::env::var("STALWART_OAUTH_HTTP_ADDR") else {
        eprintln!("skipping JMAP sign-in live test: STALWART_OAUTH_HTTP_ADDR unset");
        return;
    };
    let server = format!("http://{addr}");
    let store = RecordingStore::default();
    let app = app("signin", store.clone());

    assert!(
        app.jmap_oauth_available(EMAIL.to_owned(), Some(server.clone())),
        "the pre-flight must offer sign-in for {server}: the server publishes discovery metadata \
         and a registration endpoint",
    );

    let start = app
        .begin_jmap_login(
            EMAIL.to_owned(),
            Some(server.clone()),
            REDIRECT_URI.to_owned(),
        )
        .expect("discovery and registration succeed");
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    assert_eq!(
        format!(
            "{}{}",
            authorization.origin().ascii_serialization(),
            authorization.path()
        ),
        format!("{server}/login"),
        "the authorization endpoint is the one the server's metadata names",
    );
    assert_eq!(param(&authorization, "response_type"), "code");
    assert_eq!(param(&authorization, "redirect_uri"), REDIRECT_URI);
    assert_eq!(param(&authorization, "code_challenge_method"), "S256");
    // The resource the server published, so the token is bound to this JMAP server (RFC 8707).
    assert_eq!(param(&authorization, "resource"), server);
    assert!(
        !param(&authorization, "client_id").is_empty(),
        "registration issued a client id",
    );
    let scopes = param(&authorization, "scope");
    let scopes: Vec<&str> = scopes.split(' ').collect();
    for wanted in [
        "offline_access",
        "urn:ietf:params:oauth:scope:mail",
        "urn:ietf:params:oauth:scope:calendars",
        "urn:ietf:params:oauth:scope:contacts",
    ] {
        assert!(
            scopes.contains(&wanted),
            "`{wanted}` is asked for: {scopes:?}"
        );
    }
    // The server offers `openid` too, and the product has no use for it (docs/jmap.md rule 3).
    assert!(
        !scopes.contains(&"openid"),
        "only the scopes the product uses: {scopes:?}"
    );

    let callback = sign_in_as_the_user(&authorization, REDIRECT_URI);
    let config = app
        .complete_jmap_login(start.pending, callback)
        .expect("the code is exchanged for a grant with a refresh token");
    assert!(
        config.contains("[jmap.oauth]"),
        "the config carries the grant:\n{config}"
    );
    assert!(
        !config.contains(PASSWORD),
        "a signed-in account stores no password"
    );

    // Connecting is the proof the grant works: the core refreshes it into an access token and
    // opens the JMAP session with it.
    let row = app
        .add_account(config)
        .expect("the signed-in account connects");
    assert_eq!(row.email, EMAIL);
    let persisted = store.0.lock().expect("store mutex poisoned");
    assert!(
        persisted.iter().any(|toml| toml.contains("[jmap.oauth]")),
        "the host was asked to keep the grant, or the account is gone at the next launch",
    );
}

/// The main harness must *not* offer sign-in: the clients' setup-form tests run against it and
/// assume the password field. It stays that way because the issuer it publishes,
/// `https://mail.test.local`, resolves nowhere. Should it ever become reachable, this fails before
/// those suites do, and names the cause.
#[test]
fn the_seeded_harness_offers_no_sign_in() {
    let Ok(addr) = std::env::var("STALWART_HTTP_ADDR") else {
        eprintln!("skipping: STALWART_HTTP_ADDR unset");
        return;
    };
    let app = app("seeded", RecordingStore::default());
    assert!(
        !app.jmap_oauth_available(EMAIL.to_owned(), Some(format!("http://{addr}"))),
        "the seeded harness now offers JMAP sign-in, so every client's setup form changes for it; \
         the sign-in flow belongs on `stalwart-oauth`",
    );
}
