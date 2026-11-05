//! A Nextcloud sign-in end to end through the FFI a client calls, against a real Nextcloud: the
//! engine's fixture (`docker/nextcloud` in the engine repository, `docker compose up -d --wait`),
//! reached as `NEXTCLOUD_HTTP_ADDR=127.0.0.1:18082`. Skipped when that is unset.
//!
//! What it proves that the stand-in cannot: the server's own flow yields an app password its
//! CalDAV and CardDAV accept through the engine's adapters, the account is named by the profile
//! address, signing in again withdraws the password it replaces, and removing the account
//! withdraws its own. Remote wipe is the stand-in's (`src/nextcloud_wipe_tests.rs`): marking a
//! password for wipe takes a web session and the server's own token id.

// A test drives a runtime of its own from the test thread; see `src/lib.rs`.
#![allow(clippy::disallowed_methods)]

#[allow(
    dead_code,
    reason = "its OAuth login page helpers serve the OAuth suites"
)]
mod live_oauth;

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use base64::Engine as _;
use live_oauth::RecordingStore;
use mailcal_bindings::{AccountCapability, MailcalApp, NextcloudSignInPoll};

fn fixture() -> Option<(String, String, String)> {
    let addr = std::env::var("NEXTCLOUD_HTTP_ADDR").ok()?;
    Some((
        format!("http://{addr}"),
        std::env::var("NEXTCLOUD_USER").unwrap_or_else(|_| "alice".to_owned()),
        std::env::var("NEXTCLOUD_PASS").unwrap_or_else(|_| "nextcloud-alice-pw".to_owned()),
    ))
}

#[test]
fn a_nextcloud_sign_in_connects_signs_in_again_and_leaves_with_the_account() {
    let Some((base, user, password)) = fixture() else {
        eprintln!("skipping the Nextcloud sign-in: NEXTCLOUD_HTTP_ADDR unset");
        return;
    };
    let store = RecordingStore::default();
    let app = live_oauth::app("nextcloud-sign-in", store.clone());
    assert!(app.offers_nextcloud_sign_in(format!("{base}/remote.php/dav/")));
    // A bare host is asked once its own `.well-known` lands on `/remote.php/dav/`.
    assert!(app.offers_nextcloud_sign_in(base.clone()));

    let grant = signed_in(
        &app,
        &app.begin_nextcloud_sign_in(base.clone()).unwrap(),
        &user,
        &password,
    );
    let both = vec![AccountCapability::Calendar, AccountCapability::Contacts];
    let row = app
        .add_nextcloud_account(grant, both)
        .expect("the account connects");
    assert_eq!(
        row.email, "alice@test.local",
        "named by the profile address"
    );
    let first = stored_password(&store, &password);
    assert_eq!(dav_status(&base, &first), 207);
    let entry = app.accounts_snapshot().accounts.remove(0);
    assert!(entry.nextcloud);
    assert!(app.account_sign_in_options(entry.id.clone()).nextcloud);

    let start = app.begin_account_nextcloud_sign_in(row.id.clone()).unwrap();
    let again = signed_in(&app, &start, &user, &password);
    app.complete_account_nextcloud_sign_in(row.id.clone(), again)
        .expect("signing in again connects");
    let second = stored_password(&store, &password);
    assert_ne!(first.1, second.1, "the new password is stored");
    eventually_refused(&base, &first, "the replaced password is withdrawn");
    assert_eq!(dav_status(&base, &second), 207);

    app.remove_account(row.id).unwrap();
    eventually_refused(
        &base,
        &second,
        "the removed account's password is withdrawn",
    );
}

/// Runs the browser half of `start` as `user`, then polls until the grant.
fn signed_in(
    app: &MailcalApp,
    start: &mailcal_bindings::NextcloudSignInStart,
    user: &str,
    password: &str,
) -> String {
    assert_eq!(
        app.poll_nextcloud_sign_in(start.pending.clone()).unwrap(),
        NextcloudSignInPoll::Waiting
    );
    grant_in_the_browser(&start.login_url, user, password);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let NextcloudSignInPoll::Granted { grant } =
            app.poll_nextcloud_sign_in(start.pending.clone()).unwrap()
        {
            return grant;
        }
        assert!(Instant::now() < deadline, "the grant never arrived");
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The login name and app password the latest stored config carries; never the typed password.
fn stored_password(store: &RecordingStore, typed: &str) -> (String, String) {
    let configs = store.0.lock().unwrap();
    let latest = configs.last().expect("a stored config");
    assert!(latest.contains("[nextcloud]"), "{latest}");
    assert!(
        !latest.contains(typed),
        "the typed password is never stored"
    );
    let config: toml::Table = latest.parse().unwrap();
    let caldav = config["caldav"].as_table().unwrap();
    (
        caldav["username"].as_str().unwrap().to_owned(),
        caldav["password"].as_str().unwrap().to_owned(),
    )
}

/// The status of a depth-0 `PROPFIND` of the DAV root with `credentials`.
fn dav_status(base: &str, credentials: &(String, String)) -> u16 {
    runtime().block_on(async {
        http()
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                format!("{base}/remote.php/dav/"),
            )
            .basic_auth(&credentials.0, Some(&credentials.1))
            .header("Depth", "0")
            .send()
            .await
            .expect("the DAV root answers")
            .status()
            .as_u16()
    })
}

/// Waits for `credentials` to be refused, which a withdrawal does in the background.
fn eventually_refused(base: &str, credentials: &(String, String), what: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while dav_status(base, credentials) != 401 {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// What a person does in the browser: open the login page, sign in, grant access.
fn grant_in_the_browser(login_url: &str, user: &str, password: &str) {
    let login = url::Url::parse(login_url).unwrap();
    let origin = login.origin().ascii_serialization();
    let index = format!(
        "{origin}{}",
        login.path().split("/login/v2").next().unwrap()
    );
    let index = index.trim_end_matches("/index.php").to_owned() + "/index.php";
    runtime().block_on(async {
        let mut browser = Browser::default();
        let picker = browser.get(login_url).await;
        let auth = initial_state(&picker, "loginFlowAuth");
        let grant_page = url::Url::parse(auth["loginRedirectUrl"].as_str().unwrap()).unwrap();
        let back = format!(
            "{}?{}",
            grant_page.path(),
            grant_page.query().unwrap_or_default()
        );
        let form = browser
            .get(&format!("{index}/login?redirect_url={}", urlencode(&back)))
            .await;
        let token = request_token(&form);
        let granting = browser
            .post(
                &format!("{index}/login"),
                &[
                    ("user", user),
                    ("password", password),
                    ("requesttoken", &token),
                    ("redirect_url", &back),
                    ("timezone", "Europe/Amsterdam"),
                    ("timezone_offset", "1"),
                ],
                &origin,
                None,
            )
            .await;
        let grant = initial_state(&granting, "loginFlowGrant");
        let done = browser
            .post(
                grant["actionUrl"].as_str().unwrap(),
                &[("stateToken", grant["stateToken"].as_str().unwrap())],
                &origin,
                Some(&request_token(&granting)),
            )
            .await;
        assert!(
            done.contains("loginFlowState"),
            "the grant was not accepted"
        );
    });
}

/// A browser enough for Nextcloud's login: cookies kept, redirects followed.
#[derive(Default)]
struct Browser {
    cookies: BTreeMap<String, String>,
}

impl Browser {
    async fn get(&mut self, url: &str) -> String {
        self.send(http().get(url), url).await
    }

    async fn post(
        &mut self,
        url: &str,
        form: &[(&str, &str)],
        origin: &str,
        token: Option<&str>,
    ) -> String {
        let mut request = http().post(url).form(form).header("Origin", origin);
        if let Some(token) = token {
            request = request.header("requesttoken", token);
        }
        self.send(request, url).await
    }

    async fn send(&mut self, mut request: reqwest::RequestBuilder, url: &str) -> String {
        let mut url = url::Url::parse(url).unwrap();
        for _ in 0..8 {
            let cookie: Vec<String> = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            let response = request
                .header("Cookie", cookie.join("; "))
                .send()
                .await
                .expect("the login page answers");
            for set in response.headers().get_all("set-cookie") {
                let pair = set.to_str().unwrap().split(';').next().unwrap();
                if let Some((name, value)) = pair.split_once('=') {
                    self.cookies
                        .insert(name.trim().to_owned(), value.trim().to_owned());
                }
            }
            if response.status().is_redirection() {
                let location = response.headers()["location"].to_str().unwrap();
                url = url.join(location).unwrap();
                request = http().get(url.clone());
                continue;
            }
            let status = response.status();
            let body = response.text().await.unwrap();
            assert!(status.is_success(), "{url} answered {status}");
            return body;
        }
        panic!("too many redirects from {url}");
    }
}

fn initial_state(page: &str, key: &str) -> serde_json::Value {
    let marker = format!("id=\"initial-state-core-{key}\" value=\"");
    let start = page
        .find(&marker)
        .unwrap_or_else(|| panic!("no {key} on the page"))
        + marker.len();
    let value = &page[start..start + page[start..].find('"').unwrap()];
    let json = base64::engine::general_purpose::STANDARD
        .decode(unescape(value))
        .unwrap();
    serde_json::from_slice(&json).unwrap()
}

fn request_token(page: &str) -> String {
    let marker = "data-requesttoken=\"";
    let start = page.find(marker).expect("a request token") + marker.len();
    unescape(&page[start..start + page[start..].find('"').unwrap()])
}

fn unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn urlencode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn http() -> reqwest::Client {
    mailcal_nextcloud::client_for(&mailcal_account::setup_trust_policy()).unwrap()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().unwrap()
}
