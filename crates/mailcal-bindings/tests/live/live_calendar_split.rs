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
//!   cargo test -p mailcal-bindings --test live -- live_calendar_split:: --nocapture
//! ```

use std::time::{Duration, Instant};

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountEntry, AccountPageIntent, ImapLoginRequest,
    Intent, MailcalApp, NoticeKind, SetupRecommendation, TypedPassword,
};

use crate::live_oauth::{LOGIN, PASSWORD, RecordingStore, app, sign_in_as_the_user};

const REDIRECT_URI: &str = "mailcal.test://sign-in";

/// Waits for `until` to answer, failing with `what` after a minute.
fn wait<T>(what: &str, until: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(found) = until() {
            return found;
        }
        assert!(Instant::now() < deadline, "never {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

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

/// How many of the app's notices say a calendar, or an address book, did not connect.
fn outages(app: &MailcalApp) -> (usize, usize) {
    let notices = app.notices().notices;
    let calendar = notices
        .iter()
        .filter(|notice| matches!(notice.kind, NoticeKind::CalendarOutage { .. }))
        .count();
    let contacts = notices
        .iter()
        .filter(|notice| matches!(notice.kind, NoticeKind::ContactsOutage { .. }))
        .count();
    (calendar, contacts)
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
    assert_eq!(outages(&app).0, 1, "{:?}", app.notices().notices);

    // The page's form, as a client sends it: the section is drawn once the refusal is known.
    wait("drew the separate server's form", || {
        app.account_page(mailbox.clone())?
            .separate_server
            .map(|_| ())
    });
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::ConnectSeparateServer {
            account: mailbox.clone(),
            username: LOGIN.to_owned(),
            password: TypedPassword {
                text: PASSWORD.to_owned(),
            },
        },
    });
    let page = wait("moved the calendar to an account of its own", || {
        let page = app.account_page(mailbox.clone())?;
        if let Some(section) = &page.separate_server {
            assert!(section.failed.is_none(), "the connect failed: {section:?}");
            return None;
        }
        // Remove is held while a change runs on the page, so pressable means it has ended.
        (page.remove.enabled && page.uses.iter().any(|row| row.linked.is_some())).then_some(page)
    });
    let calendar = page
        .uses
        .iter()
        .find(|row| row.capability == AccountCapability::Calendar)
        .and_then(|row| row.linked.clone())
        .expect("the calendar row names the account it comes from")
        .id;
    assert_eq!(
        page.notice.map(|raised| raised.notice),
        Some(AccountChangeNotice::SignedInSeparately {
            uses: vec![AccountCapability::Calendar, AccountCapability::Contacts]
        }),
        "the mailbox's page says why there are two accounts now"
    );
    assert!(page.links.is_empty(), "filled slots are said on their rows");

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
    assert_eq!(outages(&app), (0, 0));

    let calendar_page = app.account_page(calendar.clone()).expect("listed");
    let [taken] = calendar_page.linked_from.as_slice() else {
        panic!("one mailbox takes it: {:?}", calendar_page.linked_from);
    };
    assert_eq!(taken.account.id, mailbox);
    assert_eq!(
        taken.leaves_without,
        [AccountCapability::Calendar, AccountCapability::Contacts],
        "the mailbox has no server of its own for them any more"
    );
    assert!(calendar_page.links.is_empty(), "its one mailbox is implied");

    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::StopLinking {
            account: calendar.clone(),
            from: mailbox.clone(),
        },
    });
    wait("stopped linking", || {
        let mail = entry(&app, &mailbox);
        (mail.links.calendar.is_none() && mail.links.contacts.is_none()).then_some(())
    });
    let unlinked = app.account_page(mailbox.clone()).expect("listed");
    assert!(unlinked.uses.iter().all(|row| row.linked.is_none()));
    assert!(
        app.account_page(calendar.clone())
            .expect("listed")
            .linked_from
            .is_empty()
    );

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
