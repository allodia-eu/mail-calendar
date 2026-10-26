//! Gated live check of switching an existing account between a password and signing in through
//! its provider, against the harness's sign-in server (`stalwart-oauth` in
//! `docker/stalwart/docker-compose.yml`), through the FFI Settings calls.
//!
//! JMAP makes the whole round: alice is added with her password, signs in instead, and goes back
//! to the password, the account keeping its id throughout. IMAP signs in, signs in again from the
//! stored grant, and is refused a password: the sign-in server holds her mailbox under
//! `alice@test.local` and takes `alice@localhost` only on a token, so the refusal is what a
//! switch to a password the server will not take must leave behind.
//!
//! Skips each half unless its variable is set; `scripts/dev/harness.sh test` sets both:
//! ```sh
//! STALWART_OAUTH_HTTP_ADDR=localhost:28081 \
//!   MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem" \
//!   MAILCAL_HARNESS_OAUTH_IMAP=localhost:12995 \
//!   cargo test -p mailcal-bindings --test live_sign_in_switch -- --nocapture
//! ```

mod live_oauth;

use live_oauth::{LOGIN, PASSWORD, RecordingStore, app, sign_in_as_the_user};
use mailcal_bindings::{
    ImapAuthOffer, ImapLoginRequest, MailcalApp, MailcalError, SetupRecommendation, SignInMethod,
};

const REDIRECT_URI: &str = "mailcal.test://sign-in";

fn method(app: &MailcalApp, id: &str) -> SignInMethod {
    app.accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.id == id)
        .expect("the account keeps its id")
        .sign_in
}

/// Runs the browser half of signing `id` in, as a client does from its Settings page.
fn sign_in(app: &MailcalApp, id: &str) {
    let start = app
        .begin_account_sign_in(id.to_owned(), REDIRECT_URI.to_owned())
        .expect("the sign-in starts");
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    let redirect = live_oauth::param(&authorization, "redirect_uri");
    let callback = sign_in_as_the_user(&authorization, &redirect);
    app.complete_account_sign_in(id.to_owned(), start.pending, callback, None)
        .expect("the grant connects and replaces the credential");
}

fn last_stored(store: &RecordingStore) -> String {
    store
        .0
        .lock()
        .expect("store mutex poisoned")
        .last()
        .cloned()
        .expect("the switch was stored")
}

#[test]
fn a_jmap_account_switches_to_signing_in_and_back() {
    let Ok(addr) = std::env::var("STALWART_OAUTH_HTTP_ADDR") else {
        eprintln!("skipping the JMAP sign-in switch: STALWART_OAUTH_HTTP_ADDR unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("switch-jmap", store.clone());
    let row = app
        .add_account(format!(
            "[jmap]\nemail = \"{LOGIN}\"\nbase_url = \"http://{addr}\"\npassword = \"{PASSWORD}\"\n"
        ))
        .expect("alice connects with her password");
    assert_eq!(method(&app, &row.id), SignInMethod::Password);

    let options = app.account_sign_in_options(row.id.clone());
    assert!(options.sign_in, "the server offers sign-in");
    assert!(options.password, "and the password field stays");

    sign_in(&app, &row.id);
    assert_eq!(method(&app, &row.id), SignInMethod::Provider);
    let stored = last_stored(&store);
    assert!(stored.contains("[jmap.oauth]"), "the grant is stored");
    assert!(!stored.contains(PASSWORD), "and the password is gone");

    app.switch_account_to_password(row.id.clone(), PASSWORD.to_owned())
        .expect("the password connects");
    assert_eq!(method(&app, &row.id), SignInMethod::Password);
    let stored = last_stored(&store);
    assert!(!stored.contains("oauth"), "the grant is gone:\n{stored}");
}

#[test]
fn an_imap_sign_in_signs_in_again_and_keeps_its_grant_over_a_refused_password() {
    let Ok(imap) = std::env::var("MAILCAL_HARNESS_OAUTH_IMAP") else {
        eprintln!("skipping the IMAP sign-in switch: MAILCAL_HARNESS_OAUTH_IMAP unset");
        return;
    };
    let store = RecordingStore::default();
    let app = app("switch-imap", store.clone());
    let SetupRecommendation::Imap {
        imap_host,
        smtp_host,
        imap_security,
        smtp_security,
        oauth_issuer,
        ..
    } = app.detect_account_settings("alice@localhost".to_owned(), None)
    else {
        panic!("alice@localhost detects as IMAP; MAILCAL_EXTRA_CA must name the harness bundle");
    };
    assert_eq!(imap_host, imap);
    let request = ImapLoginRequest {
        email: "alice@localhost".to_owned(),
        imap_host,
        smtp_host,
        caldav_base_url: None,
        imap_security: Some(imap_security),
        smtp_security: Some(smtp_security),
        oauth_issuer,
        carddav_base_url: None,
        uses: None,
    };
    assert!(matches!(
        app.imap_auth_options(request.clone()),
        ImapAuthOffer::SignIn { .. }
    ));
    let start = app
        .begin_imap_login(request, REDIRECT_URI.to_owned())
        .expect("discovery and registration succeed");
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    let callback = sign_in_as_the_user(&authorization, REDIRECT_URI);
    let config = app
        .complete_imap_login(start.pending, callback)
        .expect("the code is exchanged");
    let row = app.add_account(config).expect("the sign-in connects");
    let client_id = |toml: &str| {
        toml.lines()
            .find(|line| line.starts_with("client_id"))
            .map(str::to_owned)
    };
    let registered = client_id(&last_stored(&store));

    let options = app.account_sign_in_options(row.id.clone());
    assert!(options.sign_in, "a signed-in account can sign in again");
    assert!(
        options.password,
        "the server names PLAIN beside OAUTHBEARER"
    );

    sign_in(&app, &row.id);
    assert_eq!(method(&app, &row.id), SignInMethod::Provider);
    assert_eq!(
        client_id(&last_stored(&store)),
        registered,
        "signing in again registers no second client"
    );

    let stored_before = store.0.lock().expect("store mutex poisoned").len();
    let refused = app.switch_account_to_password(row.id.clone(), PASSWORD.to_owned());
    assert!(
        matches!(refused, Err(MailcalError::SignInRefused { .. })),
        "the server takes no password for alice@localhost: {refused:?}"
    );
    assert_eq!(method(&app, &row.id), SignInMethod::Provider);
    assert_eq!(
        store.0.lock().expect("store mutex poisoned").len(),
        stored_before,
        "nothing is stored for a password the server refused"
    );
}
