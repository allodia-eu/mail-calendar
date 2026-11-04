//! What the two sign-in live tests share: an account-less app over a recording credential store,
//! and the one step a test cannot take the way a user does, the login page.
//!
//! Both tests run against the harness's sign-in server (`stalwart-oauth` in
//! `docker/stalwart/docker-compose.yml`), where alice signs in with her harness password.

// A test drives a runtime of its own from the test thread; see `src/lib.rs`.
#![allow(clippy::disallowed_methods)]

use std::{
    fs,
    sync::{Arc, Mutex, mpsc},
};

use mailcal_bindings::{
    AccountCredentialStore, CredentialStoreError, DeviceClass, DeviceInfo, LogLevel, Logger,
    MailcalApp, Observer, Platform, Surface,
};

/// The account alice signs in to Stalwart's login page as.
pub const LOGIN: &str = "alice@test.local";
/// Her harness password (a fixture, not a secret).
pub const PASSWORD: &str = "harness-alice-pw";

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
pub struct RecordingStore(pub Arc<Mutex<Vec<String>>>);

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

/// An account-less app in a fresh temp dir of its own.
pub fn app(name: &str, store: RecordingStore) -> Arc<MailcalApp> {
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
pub fn param(url: &url::Url, name: &str) -> String {
    let mut values = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value);
    let value = values
        .next()
        .unwrap_or_else(|| panic!("the URL carries no `{name}`: {url}"));
    assert!(values.next().is_none(), "`{name}` appears twice: {url}");
    value.into_owned()
}

/// Signs in on the login page as the user would, and returns the redirect the browser would then
/// follow back to the app at `redirect_uri`.
///
/// Stalwart's `/login` page is a script that hands the authorization request it was opened with,
/// verbatim, to `/api/auth`: it asks who the client is, posts the account's credentials, and, the
/// first time a client asks for an account, posts the person's consent. The last answer carries
/// the redirect, code, state and issuer included, built by the server. This takes the same steps
/// with the query string the core built, so a parameter the core dropped or mangled fails here.
/// Those calls are Stalwart's web UI API, not a standard; if a Stalwart bump breaks them, the
/// failure names this step.
pub fn sign_in_as_the_user(authorization_url: &url::Url, redirect_uri: &str) -> String {
    let origin = authorization_url.origin().ascii_serialization();
    let query = authorization_url
        .query()
        .expect("the authorization URL carries its request")
        .to_owned();
    let runtime = tokio::runtime::Runtime::new().expect("a runtime for the login posts");
    let location = runtime.block_on(async {
        let http = mailcal_oauth::discovery_client().expect("the shared HTTP client");
        let client = login_api(http.get(format!("{origin}/api/auth/client?{query}"))).await;
        assert!(
            client.get("error").is_none(),
            "the login page refused the authorization request: {client}"
        );
        let post = |body: serde_json::Value| {
            login_api(
                http.post(format!("{origin}/api/auth"))
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            )
        };
        let mut answer = post(serde_json::json!({
            "type": "authCode",
            "accountName": LOGIN,
            "accountSecret": PASSWORD,
            "query": query,
        }))
        .await;
        if answer["type"] == "consentRequired" {
            let token = answer["token"]
                .as_str()
                .expect("a consent token")
                .to_owned();
            answer = post(serde_json::json!({
                "type": "authConsent",
                "token": token,
                "query": query,
                "consent": true,
            }))
            .await;
        }
        assert!(
            answer["type"] == "authenticated" || answer["type"] == "redirect",
            "Stalwart did not sign alice in: {answer}"
        );
        answer["location"]
            .as_str()
            .expect("the redirect back to the app")
            .to_owned()
    });
    assert!(
        location.starts_with(redirect_uri),
        "the sign-in redirects to the app's own URI ({redirect_uri}): {location}"
    );
    let callback = url::Url::parse(&location).expect("the redirect parses");
    assert_eq!(
        param(&callback, "state"),
        param(authorization_url, "state"),
        "the redirect carries the request's own state"
    );
    location
}

/// Sends one of the login page's requests and reads its JSON answer, failing on any other status.
async fn login_api(request: reqwest::RequestBuilder) -> serde_json::Value {
    let response = request
        .header("Accept", "application/json")
        .send()
        .await
        .expect("the login page's API answers");
    let status = response.status();
    let text = response.text().await.expect("a readable login answer");
    assert!(
        status.is_success(),
        "the login page's API refused ({status}): {text}"
    );
    serde_json::from_str(&text).expect("the login answer is JSON")
}
