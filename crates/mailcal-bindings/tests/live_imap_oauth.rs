//! Gated live check of "Sign in with your provider" for an IMAP account, through the public FFI a
//! client calls, in the order a setup screen calls it: `detect_account_settings`, the
//! `imap_auth_options` pre-flight, `begin_imap_login`, `complete_imap_login` and `add_account`.
//!
//! What runs for real: the provider's own HTTPS autoconfig naming an issuer, the IMAP capability
//! probe, RFC 8414 metadata, RFC 7591 registration, the PKCE authorization request, the code
//! exchange, and an account connected over OAUTHBEARER on the resulting grant. The server is the
//! harness's sign-in server as its HTTPS front presents it (`stalwart-oauth` and
//! `stalwart-oauth-front` in `docker/stalwart/docker-compose.yml`; what the front rewrites, and
//! why, is in `docker/stalwart/front/nginx.conf`). The login page is posted to directly, as in
//! the JMAP test.
//!
//! Skips unless `MAILCAL_HARNESS_OAUTH_IMAP` is set. `MAILCAL_EXTRA_CA` must name the harness's
//! certificate bundle, or detection finds no trusted autoconfig and every step after it is a
//! password form. `scripts/dev/harness.sh test` sets both; by hand:
//! ```sh
//! scripts/dev/harness.sh up
//! MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem" \
//!   MAILCAL_HARNESS_OAUTH_IMAP=localhost:12995 \
//!   cargo test -p mailcal-bindings --test live_imap_oauth -- --nocapture
//! ```

mod live_oauth;

use live_oauth::{PASSWORD, RecordingStore, app, optional_param, param, sign_in_as_the_user};
use mailcal_bindings::{ImapAuthOffer, ImapLoginRequest, SetupRecommendation};

/// The address whose autoconfig the front serves at `https://localhost`.
const EMAIL: &str = "alice@localhost";

/// The issuer that autoconfig names, and the name the front republishes the metadata under.
const ISSUER: &str = "https://127.0.0.1";

/// A custom-scheme redirect of the shape every client registers (`<app id>://imap-oauth`).
const REDIRECT_URI: &str = "mailcal.test://imap-oauth";

#[test]
fn an_imap_account_signs_in_through_its_providers_own_autoconfig() {
    let Ok(imap) = std::env::var("MAILCAL_HARNESS_OAUTH_IMAP") else {
        eprintln!("skipping IMAP sign-in live test: MAILCAL_HARNESS_OAUTH_IMAP unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("imap-signin", store.clone());

    // The provider describing itself: detection reads the autoconfig over trusted HTTPS, so the
    // issuer it names is carried through to the setup form (docs/mail-oauth.md rule 4).
    let SetupRecommendation::Imap {
        imap_host,
        smtp_host,
        imap_security,
        smtp_security,
        oauth_issuer,
        ..
    } = app.detect_account_settings(EMAIL.to_owned(), None)
    else {
        panic!(
            "{EMAIL} must detect as IMAP from https://localhost; a Manual route here usually \
             means MAILCAL_EXTRA_CA does not name the harness's bundle"
        );
    };
    assert_eq!(imap_host, imap, "the sign-in server's IMAP listener");
    assert_eq!(
        oauth_issuer.as_deref(),
        Some(ISSUER),
        "a trusted autoconfig's issuer reaches the setup form",
    );

    let request = ImapLoginRequest {
        email: EMAIL.to_owned(),
        imap_host,
        smtp_host,
        caldav_base_url: None,
        imap_security: Some(imap_security),
        smtp_security: Some(smtp_security),
        oauth_issuer,
    };
    assert_eq!(
        app.imap_auth_options(request.clone()),
        ImapAuthOffer::SignIn {
            issuer: ISSUER.to_owned(),
            provider_label: None,
            password_also_works: true,
        },
        "the server advertises OAUTHBEARER and PLAIN, and its issuer offers open registration",
    );

    let start = app
        .begin_imap_login(request, REDIRECT_URI.to_owned())
        .expect("discovery and registration succeed");
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    assert_eq!(
        format!(
            "{}{}",
            authorization.origin().ascii_serialization(),
            authorization.path()
        ),
        "http://localhost:28081/login",
        "the authorization endpoint is the one the metadata names, not the front",
    );
    assert_eq!(param(&authorization, "response_type"), "code");
    assert_eq!(param(&authorization, "redirect_uri"), REDIRECT_URI);
    assert_eq!(param(&authorization, "code_challenge_method"), "S256");
    assert_eq!(
        optional_param(&authorization, "resource"),
        None,
        "an IMAP account sends no resource indicator (docs/mail-oauth.md)",
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

    let callback = sign_in_as_the_user(&authorization, REDIRECT_URI);
    let config = app
        .complete_imap_login(start.pending, callback)
        .expect("the code is exchanged for a grant with a refresh token");
    assert!(
        config.contains("[oauth]"),
        "the grant is stored once, at the root:\n{config}"
    );
    assert!(
        !config.contains(PASSWORD),
        "a signed-in account stores no password"
    );

    // Connecting is the proof the grant works: the core refreshes it into an access token and
    // presents it over SASL to the IMAP server it was minted for.
    let row = app
        .add_account(config)
        .expect("the signed-in account connects over OAUTHBEARER");
    assert_eq!(row.email, EMAIL);
    let persisted = store.0.lock().expect("store mutex poisoned");
    assert!(
        persisted.iter().any(|toml| toml.contains("[oauth]")),
        "the host was asked to keep the grant, or the account is gone at the next launch",
    );
}
