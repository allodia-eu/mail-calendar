//! What the two sign-in live tests share: an account-less app over a recording credential store,
//! and the one step a test cannot take the way a user does, the login page.
//!
//! Both tests run against the harness's sign-in server (`stalwart-oauth` in
//! `docker/stalwart/docker-compose.yml`), where alice signs in with her harness password.

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
    optional_param(url, name)
        .unwrap_or_else(|| panic!("the authorization URL carries no `{name}`: {url}"))
}

/// The single value of query parameter `name`, or `None` when it is absent. Fails the test when
/// it is repeated.
pub fn optional_param(url: &url::Url, name: &str) -> Option<String> {
    let mut values = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value);
    let value = values.next()?;
    assert!(values.next().is_none(), "`{name}` appears twice: {url}");
    Some(value.into_owned())
}

/// Signs in on the login page as the user would, and returns the redirect the browser would then
/// follow back to the app at `redirect_uri`.
///
/// Stalwart's `/login` page is a web app that posts the account's credentials, together with the
/// authorization request it was opened with, to `/api/auth`, and redirects to `redirect_uri` with
/// the code it gets back. This makes that same post. Everything in it is read from the
/// authorization URL the core built, so a parameter the core dropped or mangled fails here. That
/// call is Stalwart's web UI API, not a standard; if a Stalwart bump breaks it, the failure names
/// this step.
pub fn sign_in_as_the_user(authorization_url: &url::Url, redirect_uri: &str) -> String {
    let origin = authorization_url.origin().ascii_serialization();
    let mut body = serde_json::json!({
        "type": "authCode",
        "accountName": LOGIN,
        "accountSecret": PASSWORD,
        "clientId": param(authorization_url, "client_id"),
        "redirectUri": param(authorization_url, "redirect_uri"),
        "scope": param(authorization_url, "scope"),
        "codeChallenge": param(authorization_url, "code_challenge"),
        "codeChallengeMethod": param(authorization_url, "code_challenge_method"),
        "state": param(authorization_url, "state"),
    });
    // A JMAP request names the resource it wants a token for (RFC 8707); an IMAP one has no URI
    // to name, and sends none.
    if let Some(resource) = optional_param(authorization_url, "resource") {
        body["resource"] = serde_json::json!([resource]);
    }
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

    let mut callback = url::Url::parse(redirect_uri).expect("the redirect URI parses");
    callback
        .query_pairs_mut()
        .append_pair("code", code)
        .append_pair("state", &param(authorization_url, "state"))
        .append_pair("iss", issuer);
    callback.into()
}
