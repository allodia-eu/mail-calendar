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
//!   cargo test -p mailcal-bindings --test live -- live_calendar_sign_in:: --nocapture
//! ```

use std::time::{Duration, Instant};

use mailcal_bindings::{
    AccountCapability, AccountChangeNotice, AccountEntry, AccountPageIntent, AccountSeparateSignIn,
    BrowserSignInStage, CapabilityState, Intent, MailcalApp, NoticeKind, SignInBrowserEnd,
    SignInMethod,
};

use crate::live_oauth::{self, LOGIN, PASSWORD, RecordingStore, app, sign_in_as_the_user};

const REDIRECT_URI: &str = "mailcal.test://sign-in";

/// Where the front serves the sign-in server's calendars and contacts: its own host name, which
/// its issuer lists among the origins its tokens are for.
const SIGN_IN_SERVER: &str = "https://localhost";

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

/// Waits for `until` to answer, failing with `what` after the sign-in's own time to finish.
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

/// The attempt and the page it opens, once the core waits on the browser for it.
fn in_browser(app: &MailcalApp) -> (u64, String) {
    wait("waited on the browser", || {
        let sign_in = app.browser_sign_in()?;
        match sign_in.stage {
            BrowserSignInStage::InBrowser { url, .. } => Some((sign_in.attempt, url)),
            _ => None,
        }
    })
}

/// The account the mailbox's calendar moved to, once the sign-in has ended and its form is gone.
fn moved_to(app: &MailcalApp, mailbox: &str) -> String {
    wait("moved the calendar to an account of its own", || {
        let page = app.account_page(mailbox.to_owned())?;
        // Remove is held while a change runs on the page, so pressable means it has ended.
        if app.browser_sign_in().is_some() || !page.remove.enabled {
            return None;
        }
        if let Some(section) = page.separate_server {
            assert!(section.failed.is_none(), "the sign-in failed: {section:?}");
            return None;
        }
        entry(app, mailbox).links.calendar.map(|to| to.id)
    })
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

    // The page asks the server what it offers when it is drawn, as a client's form is.
    wait(
        "offered the calendar server's own sign-in on the page",
        || {
            let section = app.account_page(mailbox.clone())?.separate_server?;
            (section.uses == both && section.sign_in == Some(AccountSeparateSignIn::Provider))
                .then_some(())
        },
    );
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInSeparateServer {
            account: mailbox.clone(),
            redirect_uri: REDIRECT_URI.to_owned(),
        },
    });
    let (attempt, url) = in_browser(&app);
    let authorization = url::Url::parse(&url).expect("a well-formed URL");
    let scope = live_oauth::param(&authorization, "scope");
    assert!(
        scope.contains("calendars") && scope.contains("contacts") && !scope.contains("mail"),
        "asks for what moves and no mail: {scope}"
    );
    let callback_url = sign_in_as_the_user(&authorization, REDIRECT_URI);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInBrowserEnded {
            account: mailbox.clone(),
            attempt,
            end: SignInBrowserEnd::Redirected { callback_url },
        },
    });
    let calendar = moved_to(&app, &mailbox);
    assert_eq!(
        app.account_page(mailbox.clone())
            .and_then(|page| page.notice)
            .map(|raised| raised.notice),
        Some(AccountChangeNotice::SignedInSeparately {
            uses: vec![AccountCapability::Calendar, AccountCapability::Contacts]
        }),
        "the mailbox's page says why there are two accounts now"
    );

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
    assert!(
        app.account_page(mail.id.clone())
            .expect("listed")
            .separate_server
            .is_none()
    );
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
    assert_eq!(outages(&app), (0, 0));
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
