//! Gated live check of "Sign in with your provider" for a JMAP account, through the public FFI a
//! client calls: the pre-flight, `begin_jmap_login`, `complete_jmap_login` and `add_account`,
//! against the harness's sign-in server (`stalwart-oauth` in `docker/stalwart/docker-compose.yml`).
//!
//! What runs for real: RFC 9728 discovery from the session URL, RFC 8414 metadata, RFC 7591
//! registration, the PKCE authorization request with its RFC 8707 resource indicator, the code
//! exchange, and an account connected on the resulting grant. The one step a test cannot take the
//! way a user does is the login page, so [`sign_in_as_the_user`] posts the page's own form to
//! Stalwart instead. That call is Stalwart's web UI API, not a standard; if a Stalwart bump breaks
//! it, the failure names that step.
//!
//! Skips unless `STALWART_OAUTH_HTTP_ADDR` is set, so the offline `cargo test` stays green. Run
//! locally:
//! ```sh
//! (cd docker/stalwart && docker compose up -d --wait)
//! STALWART_OAUTH_HTTP_ADDR=localhost:28081 STALWART_HTTP_ADDR=127.0.0.1:28080 \
//!   cargo test -p mailcal-bindings --test live_jmap_oauth -- --nocapture
//! ```

use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
};

use mailcal_bindings::{
    AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo, LogLevel, Logger,
    MailcalApp, Observer, Platform, Surface,
};

const EMAIL: &str = "alice@test.local";
const PASSWORD: &str = "harness-alice-pw";

/// A custom-scheme redirect of the shape every client registers (`<app id>://jmap-oauth`). Nothing
/// dereferences it: the code travels back in the callback URL this test builds.
const REDIRECT_URI: &str = "mailcal.test://jmap-oauth";

struct SilentLogger;

impl Logger for SilentLogger {
    fn log(&self, _level: LogLevel, _target: String, _message: String) {}
}

struct SilentObserver(mpsc::Sender<()>);

impl Observer for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {
        let _ = self.0.send(());
    }
}

/// Keeps every config the core asks the host to persist, as a platform keystore would.
#[derive(Clone, Default)]
struct RecordingStore(Arc<Mutex<Vec<String>>>);

impl AccountCredentialStore for RecordingStore {
    fn persist(
        &self,
        _account_id: String,
        config_toml: String,
    ) -> Result<(), CredentialStoreError> {
        self.0
            .lock()
            .expect("store mutex poisoned")
            .push(config_toml);
        Ok(())
    }

    fn delete(&self, _account_id: String) -> Result<(), CredentialStoreError> {
        Ok(())
    }
}

fn app(name: &str, store: RecordingStore) -> Arc<MailcalApp> {
    let dir =
        std::env::temp_dir().join(format!("mailcal-live-oauth-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a writable temp dir");
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_accounts(
        Box::new(SilentObserver(tx)),
        Box::new(SilentLogger),
        LogLevel::Info,
        Vec::new(),
        dir.to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        DeviceInfo {
            platform: Platform::Macos,
            os_version: "15.0".to_owned(),
            device_class: DeviceClass::MacLaptop,
            app_version: "0.0.0".to_owned(),
            locale: "en".to_owned(),
        },
        Box::new(store),
    )
    .expect("an account-less app boots")
}

/// The single value of query parameter `name`, failing the test when it is absent or repeated.
fn param(url: &url::Url, name: &str) -> String {
    let mut values = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value);
    let value = values
        .next()
        .unwrap_or_else(|| panic!("the authorization URL carries no `{name}`: {url}"));
    assert!(values.next().is_none(), "`{name}` appears twice: {url}");
    value.into_owned()
}

/// Signs in on the login page as the user would, and returns the redirect the browser would then
/// follow back to the app.
///
/// Stalwart's `/login` page is a web app that posts the account's credentials, together with the
/// authorization request it was opened with, to `/api/auth`, and redirects to `redirect_uri` with
/// the code it gets back. This makes that same post. Everything in it is read from the
/// authorization URL the core built, so a parameter the core dropped or mangled fails here.
fn sign_in_as_the_user(authorization_url: &url::Url) -> String {
    let origin = authorization_url.origin().ascii_serialization();
    let body = serde_json::json!({
        "type": "authCode",
        "accountName": EMAIL,
        "accountSecret": PASSWORD,
        "clientId": param(authorization_url, "client_id"),
        "redirectUri": param(authorization_url, "redirect_uri"),
        "scope": param(authorization_url, "scope"),
        "codeChallenge": param(authorization_url, "code_challenge"),
        "codeChallengeMethod": param(authorization_url, "code_challenge_method"),
        "state": param(authorization_url, "state"),
        "resource": [param(authorization_url, "resource")],
    });
    let runtime = tokio::runtime::Runtime::new().expect("a runtime for the login post");
    let answer: serde_json::Value = runtime.block_on(async {
        let http = mailcal_oauth::discovery_client().expect("the shared HTTP client");
        let response = http
            .post(format!("{origin}/api/auth"))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .expect("the login page's API answers");
        let status = response.status();
        let text = response.text().await.expect("a readable login answer");
        assert!(
            status.is_success(),
            "the login post was refused ({status}): {text}"
        );
        serde_json::from_str(&text).expect("the login answer is JSON")
    });
    assert_eq!(
        answer["type"], "authenticated",
        "Stalwart did not sign alice in: {answer}"
    );
    let code = answer["client_code"]
        .as_str()
        .expect("an authorization code");
    let issuer = answer["iss"].as_str().expect("the issuer (RFC 9207)");

    let mut callback = url::Url::parse(REDIRECT_URI).expect("the redirect URI parses");
    callback
        .query_pairs_mut()
        .append_pair("code", code)
        .append_pair("state", &param(authorization_url, "state"))
        .append_pair("iss", issuer);
    callback.into()
}

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

    let callback = sign_in_as_the_user(&authorization);
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
