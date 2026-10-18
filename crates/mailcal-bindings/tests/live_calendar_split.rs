//! Gated live check of an account whose calendar and contacts are on a server its sign-in does
//! not reach: alice signs in at the harness's sign-in server (`stalwart-oauth`), with her
//! calendar and contacts at the seeded server named as `localhost`, a host the issuer at
//! `127.0.0.1` vouches for no token to. The sign-in says so before the browser opens, and the
//! calendar then moves to an account of its own with the seeded server's password, linked to the
//! mailbox (`docs/accounts.md` rule 18).
//!
//! Skips unless both variables are set; `scripts/dev/harness.sh test` sets them:
//! ```sh
//! STALWART_HTTP_ADDR=127.0.0.1:28080 \
//!   MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem" \
//!   MAILCAL_HARNESS_OAUTH_IMAP=localhost:12995 \
//!   cargo test -p mailcal-bindings --test live_calendar_split -- --nocapture
//! ```

mod live_oauth;

use live_oauth::{LOGIN, PASSWORD, RecordingStore, app, sign_in_as_the_user};
use mailcal_bindings::{
    AccountCapability, AccountEntry, ImapLoginRequest, MailcalApp, SetupRecommendation,
};

const REDIRECT_URI: &str = "mailcal.test://sign-in";

fn entry(app: &MailcalApp, id: &str) -> AccountEntry {
    app.accounts_snapshot()
        .accounts
        .into_iter()
        .find(|entry| entry.id == id)
        .expect("the account is listed")
}

fn used_for(entry: &AccountEntry) -> Vec<AccountCapability> {
    entry
        .uses
        .iter()
        .filter(|used| used.state == mailcal_bindings::CapabilityState::On)
        .map(|used| used.capability)
        .collect()
}

#[test]
fn a_calendar_the_sign_in_does_not_reach_moves_to_an_account_of_its_own() {
    let (Ok(dav), Ok(imap)) = (
        std::env::var("STALWART_HTTP_ADDR"),
        std::env::var("MAILCAL_HARNESS_OAUTH_IMAP"),
    ) else {
        eprintln!(
            "skipping the calendar split: STALWART_HTTP_ADDR or MAILCAL_HARNESS_OAUTH_IMAP unset"
        );
        return;
    };
    let port = dav.rsplit(':').next().expect("an address with a port");
    let dav_url = format!("http://localhost:{port}");
    let app = app("calendar-split", RecordingStore::default());
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
        caldav_base_url: Some(dav_url.clone()),
        imap_security: Some(imap_security),
        smtp_security: Some(smtp_security),
        oauth_issuer,
        carddav_base_url: None,
        uses: None,
    };
    let start = app
        .begin_imap_login(request, REDIRECT_URI.to_owned())
        .expect("discovery and registration succeed");
    assert_eq!(
        start.unreached,
        [AccountCapability::Calendar, AccountCapability::Contacts],
        "said before the browser opens"
    );
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    let callback = sign_in_as_the_user(&authorization, REDIRECT_URI);
    let config = app
        .complete_imap_login(start.pending, callback)
        .expect("the code is exchanged");
    let mailbox = app.add_account(config).expect("the mailbox connects").id;
    let outages = app.connectivity();
    assert_eq!(
        outages.calendar_outages.len(),
        1,
        "{:?}",
        outages.calendar_outages
    );

    let calendar = app
        .add_linked_calendar_account(
            mailbox.clone(),
            vec![AccountCapability::Calendar, AccountCapability::Contacts],
            LOGIN.to_owned(),
            PASSWORD.to_owned(),
        )
        .expect("the seeded server takes alice's password");

    let mail = entry(&app, &mailbox);
    assert_eq!(used_for(&mail), [AccountCapability::Mail]);
    let linked =
        |link: &Option<mailcal_bindings::LinkedAccount>| link.as_ref().map(|to| to.id.clone());
    assert_eq!(
        linked(&mail.links.calendar).as_deref(),
        Some(calendar.as_str())
    );
    assert_eq!(
        linked(&mail.links.contacts).as_deref(),
        Some(calendar.as_str())
    );
    let moved = entry(&app, &calendar);
    assert_eq!(
        used_for(&moved),
        [AccountCapability::Calendar, AccountCapability::Contacts]
    );
    assert!(
        moved.uses.iter().all(|used| used.problem.is_none()),
        "{:?}",
        moved.uses
    );
    let outages = app.connectivity();
    assert!(outages.calendar_outages.is_empty() && outages.contacts_outages.is_empty());

    let refused = app.add_linked_calendar_account(
        calendar,
        vec![AccountCapability::Calendar],
        LOGIN.to_owned(),
        PASSWORD.to_owned(),
    );
    assert!(
        refused.is_err(),
        "a calendar account keeps no calendar on another server"
    );
}
