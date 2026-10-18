//! Gated live check of a calendar server that signs in through its own provider: alice's mailbox
//! is the seeded server's, over IMAP with its password, and her calendar and contacts are at the
//! harness's sign-in server (`stalwart-oauth`), which takes none of that. The server names its
//! issuer (RFC 9728), alice signs in there, and her calendar and contacts move to an account of
//! their own holding that grant, linked to the mailbox (`docs/accounts.md` rule 18).
//!
//! Skips unless both variables are set; `scripts/dev/harness.sh test` sets them:
//! ```sh
//! MAILCAL_EXTRA_CA="$PWD/docker/stalwart/tls/harness-ca.pem" \
//!   MAILCAL_HARNESS_IMAP=localhost:12993 \
//!   cargo test -p mailcal-bindings --test live_calendar_sign_in -- --nocapture
//! ```

mod live_oauth;

use live_oauth::{LOGIN, PASSWORD, RecordingStore, app, sign_in_as_the_user};
use mailcal_bindings::{
    AccountCapability, AccountEntry, CapabilityState, MailcalApp, SignInMethod,
};

const REDIRECT_URI: &str = "mailcal.test://sign-in";

/// The sign-in server's own origin, which is its issuer's too.
const SIGN_IN_SERVER: &str = "https://127.0.0.1";

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
        .filter(|used| used.state == CapabilityState::On)
        .map(|used| used.capability)
        .collect()
}

#[test]
fn a_calendar_server_with_a_sign_in_of_its_own_is_signed_in_to_there() {
    let (Ok(imap), Ok(_)) = (
        std::env::var("MAILCAL_HARNESS_IMAP"),
        std::env::var("MAILCAL_EXTRA_CA"),
    ) else {
        eprintln!("skipping the calendar sign-in: MAILCAL_HARNESS_IMAP or MAILCAL_EXTRA_CA unset");
        return;
    };
    let host = imap
        .rsplit_once(':')
        .map_or(imap.as_str(), |(host, _)| host);
    let mailbox_config = format!(
        r#"
[imap]
addr = "{imap}"
server_name = "{host}"
username = "{LOGIN}"
password = "{PASSWORD}"

[caldav]
base_url = "{SIGN_IN_SERVER}"
username = "{LOGIN}"
password = "not-this-servers"
"#
    );
    let app = app("calendar-sign-in", RecordingStore::default());
    let mailbox = app
        .add_account(mailbox_config)
        .expect("the mailbox connects")
        .id;
    let both = vec![AccountCapability::Calendar, AccountCapability::Contacts];

    assert!(
        app.separate_server_offers_sign_in(mailbox.clone(), both.clone()),
        "the calendar server names an issuer whose tokens are for it"
    );
    let start = app
        .begin_separate_server_sign_in(mailbox.clone(), both, REDIRECT_URI.to_owned())
        .expect("discovery and registration succeed");
    let authorization = url::Url::parse(&start.authorization_url).expect("a well-formed URL");
    let scope = live_oauth::param(&authorization, "scope");
    assert!(
        scope.contains("calendars") && scope.contains("contacts") && !scope.contains("mail"),
        "asks for what moves and no mail: {scope}"
    );
    let callback = sign_in_as_the_user(&authorization, REDIRECT_URI);
    let calendar = app
        .complete_separate_server_sign_in(mailbox.clone(), start.pending, callback)
        .expect("the calendar account connects with the grant");

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
    assert!(mail.separate_server.is_none());
    let moved = entry(&app, &calendar);
    assert_eq!(moved.sign_in, SignInMethod::Provider);
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
    assert!(
        app.calendars()
            .iter()
            .any(|row| row.account == calendar && row.can_write),
        "the calendar opened with the grant, as a calendar alice can write to: {:?}",
        app.calendars()
            .iter()
            .map(|row| (row.account.clone(), row.name.clone()))
            .collect::<Vec<_>>()
    );
}
