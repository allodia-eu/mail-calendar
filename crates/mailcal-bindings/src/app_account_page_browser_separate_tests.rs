//! A mailbox's separate server signed in through the browser, through `dispatch`, against a
//! Nextcloud stand-in that refuses every DAV password: offered on the section, waiting on the
//! browser while the core asks the server, and what it came to said on the section, the grant
//! kept for the next press. A sign-in that moves the uses is the live suites'.

use std::time::{Duration, Instant};

use crate::{
    AccountCapability, AccountChangeRefusal, AccountPageIntent, AccountSeparateServer,
    AccountSeparateSignIn, BrowserSignInStage, Intent, MailcalApp,
    nextcloud::nextcloud_tests::{STARTED, STATUS},
    nextcloud_mock::{MockNextcloud, boot, mock_nextcloud},
    tests_boot::note_after_boot_dial,
};

const GRANTED: &str = r#"{"server":"{base}","loginName":"alice","appPassword":"app-password"}"#;

/// A mailbox whose calendar is at `server`, which wants a sign-in of its own.
fn mailbox_at(name: &str, server: &MockNextcloud) -> (std::sync::Arc<MailcalApp>, String) {
    let mailbox = format!(
        "[imap]\naddr = \"127.0.0.1:1\"\nserver_name = \"127.0.0.1\"\nusername = \"alice@example.org\"\npassword = \"typed\"\n\
         \n[caldav]\nbase_url = \"{}/remote.php/dav/\"\nusername = \"alice\"\npassword = \"typed\"\n",
        server.base
    );
    let (app, _) = boot(name, vec![mailbox]);
    let id = app.accounts_snapshot().accounts[0].id.clone();
    note_after_boot_dial(
        &app,
        &id,
        mailcal_app::UseProblems {
            calendar: Some(mailcal_app::UseProblem::SignInRefused),
            contacts: None,
        },
    );
    (app, id)
}

/// The section once `until` holds of it and no change runs on the page; `what` names the wait.
fn section_when(
    app: &MailcalApp,
    id: &str,
    what: &str,
    until: impl Fn(&AccountSeparateServer) -> bool,
) -> AccountSeparateServer {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        // Idle before the read and after it, so the section was drawn with nothing running.
        let idle = !app.account_work.of(id).0.busy;
        let section = app
            .account_page(id.to_owned())
            .and_then(|page| page.separate_server);
        if let Some(section) = section.clone().filter(|section| until(section))
            && idle
            && !app.account_work.of(id).0.busy
        {
            return section;
        }
        assert!(
            Instant::now() < deadline,
            "the section never got to {what}: {section:?}, the sign-in {:?}",
            app.browser_sign_in()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn sign_in(app: &MailcalApp, id: &str) {
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInSeparateServer {
            account: id.to_owned(),
            redirect_uri: String::new(),
        },
    });
}

fn routes() -> Vec<(&'static str, u16, &'static str)> {
    vec![
        ("GET /status.php ", 200, STATUS),
        ("POST /index.php/login/v2 ", 200, STARTED),
        ("POST /login/v2/poll ", 200, GRANTED),
        ("PROPFIND ", 401, ""),
        ("DELETE /ocs/v2.php/core/apppassword ", 200, ""),
    ]
}

#[test]
fn a_nextcloud_sign_in_waits_on_the_section_and_a_grant_that_does_not_connect_is_kept() {
    let server = mock_nextcloud(routes());
    let (app, id) = mailbox_at("separate-browser-nextcloud", &server);
    let offered = section_when(&app, &id, "offered", |section| {
        section.sign_in == Some(AccountSeparateSignIn::Nextcloud)
    });
    assert_eq!(offered.uses, [AccountCapability::Calendar]);
    assert!(offered.sign_in_control.enabled && offered.browser.is_none());

    sign_in(&app, &id);
    let waiting = section_when(&app, &id, "waiting", |section| {
        matches!(
            section.browser.as_ref().map(|browser| &browser.stage),
            Some(BrowserSignInStage::InBrowser {
                redirect_uri: None,
                ..
            })
        )
    });
    assert!(!waiting.connect.enabled, "Connect is held while it is out");
    assert!(
        app.account_page(id.clone())
            .and_then(|page| page.sign_in)
            .is_none_or(|section| section.browser.is_none()),
        "the account's own sign-in section does not draw it"
    );

    let failed = section_when(&app, &id, "failed", |section| {
        section.browser.is_none() && section.failed.is_some()
    });
    assert!(
        !matches!(
            failed.failed,
            Some(AccountChangeRefusal::BrowserNotOpened | AccountChangeRefusal::NotFinished)
        ),
        "the server's answer to the granted password is said: {:?}",
        failed.failed
    );
    assert!(failed.sign_in_control.enabled && failed.connect.enabled);
    assert_eq!(
        app.account_page_notice(id.clone()),
        None,
        "the section says it"
    );
    assert!(
        server
            .requests_to("DELETE /ocs/v2.php/core/apppassword")
            .is_empty(),
        "a grant that may yet connect is kept, not withdrawn"
    );

    sign_in(&app, &id);
    section_when(&app, &id, "kept", |section| {
        section.browser.is_none() && section.failed.is_some()
    });
    assert_eq!(
        server.requests_to("POST /index.php/login/v2").len(),
        1,
        "the next press completes the kept grant without the browser"
    );
}

#[test]
fn a_cancel_ends_the_sign_in_and_the_section_says_nothing() {
    let server = mock_nextcloud(vec![
        ("GET /status.php ", 200, STATUS),
        ("POST /index.php/login/v2 ", 200, STARTED),
        ("POST /login/v2/poll ", 404, ""),
        ("PROPFIND ", 401, ""),
    ]);
    let (app, id) = mailbox_at("separate-browser-cancel", &server);
    section_when(&app, &id, "offered2", |section| section.sign_in.is_some());
    sign_in(&app, &id);
    let waiting = section_when(&app, &id, "waiting2", |section| section.browser.is_some());
    let attempt = waiting.browser.expect("waiting").attempt;
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::CancelSignIn {
            account: id.clone(),
            attempt,
        },
    });
    let after = section_when(&app, &id, "cancelled", |section| section.browser.is_none());
    assert_eq!(after.failed, None);
    assert!(after.connect.enabled && after.sign_in_control.enabled);
    assert_eq!(app.browser_sign_in(), None);
}
