//! An account's browser sign-in through `dispatch`: started by the core, waiting on the browser
//! where the client listens, and ending in what the page and the notices say. The account signs
//! in at an issuer that does not answer, so no sign-in completes; what is under test is every
//! way one ends.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountChangeNotice, AccountChangeRefusal, AccountPageIntent, BrowserSignIn,
    BrowserSignInStage, Intent, LogLevel, MailcalApp, NoticeAction, SignInBrowserEnd,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const REDIRECT: &str = "mailcal.test://imap-oauth";

/// An IMAP account signed in through its provider, at a server and an issuer that do not
/// answer: signing it in again needs no discovery.
fn signed_in_account() -> (String, String) {
    let config = mailcal_account::AccountConfig {
        imap: Some(mailcal_account::ImapAccount {
            addr: "127.0.0.1:1".to_owned(),
            server_name: "127.0.0.1".to_owned(),
            username: "alice@example.com".to_owned(),
            password: None,
            security: mailcal_account::ConnectionSecurity::ImplicitTls,
        }),
        smtp: None,
        caldav: None,
        carddav: None,
        oauth: Some(mailcal_account::OAuthGrant {
            client_id: "client-abc".to_owned(),
            client_secret: None,
            refresh_token: mailcal_account::Secret::new("stored-refresh".to_owned()),
            authorize_endpoint: "http://127.0.0.1:1/authorize".to_owned(),
            token_endpoint: "http://127.0.0.1:1/token".to_owned(),
            redirect_uri: REDIRECT.to_owned(),
            scopes: vec!["offline_access".to_owned()],
            resource: None,
            issuer: None,
            authorization_server: None,
            protected_resources: Vec::new(),
            signed_in_as: None,
        }),
        certificate_exceptions: Vec::new(),
        shape: mailcal_account::AccountShape::default(),
        dav_starts: mailcal_account::DavStarts::default(),
        nextcloud: None,
    };
    let id = config.account_id().expect("a valid account id");
    (
        config.to_toml().expect("serializable config"),
        id.as_str().to_owned(),
    )
}

fn booted(name: &str) -> (Arc<MailcalApp>, String) {
    let (config, id) = signed_in_account();
    let (tx, _rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        vec![config],
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("the app boots with a placeholder for an account it cannot reach");
    (app, id)
}

fn send(app: &MailcalApp, intent: AccountPageIntent) {
    app.dispatch(Intent::AccountPage { intent });
}

fn sign_in(app: &MailcalApp, id: &str) {
    send(
        app,
        AccountPageIntent::SignIn {
            account: id.to_owned(),
            adding: Vec::new(),
            redirect_uri: "mailcal.test://ignored".to_owned(),
        },
    );
}

/// Waits until `until` holds of the sign-in out, and answers it.
fn waits_for(
    app: &MailcalApp,
    until: impl Fn(Option<&BrowserSignIn>) -> bool,
) -> Option<BrowserSignIn> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let out = app.browser_sign_in();
        if until(out.as_ref()) && !app.account_work.of("").0.busy {
            return out;
        }
        assert!(Instant::now() < deadline, "the sign-in never got there");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn in_browser(app: &MailcalApp) -> BrowserSignIn {
    waits_for(app, |out| {
        matches!(
            out.map(|out| &out.stage),
            Some(BrowserSignInStage::InBrowser { .. })
        )
    })
    .expect("waiting on the browser")
}

fn over(app: &MailcalApp) {
    waits_for(app, |out| out.is_none());
}

fn said(app: &MailcalApp, id: &str) -> Option<AccountChangeNotice> {
    app.account_page_notice(id.to_owned())
        .map(|raised| raised.notice)
}

fn failed(refusal: AccountChangeRefusal) -> Option<AccountChangeNotice> {
    Some(AccountChangeNotice::SignInFailed {
        switching: false,
        refusal,
    })
}

#[test]
fn the_browser_returns_where_the_grant_was_registered_and_the_page_says_it_waits() {
    let (app, id) = booted("browser-sign-in-waits");
    sign_in(&app, &id);
    let out = in_browser(&app);
    assert_eq!(out.account, id);
    assert!(out.cancel.enabled);
    let BrowserSignInStage::InBrowser { url, redirect_uri } = out.stage else {
        unreachable!()
    };
    assert!(url.starts_with("http://127.0.0.1:1/authorize"));
    assert_eq!(
        redirect_uri.as_deref(),
        Some(REDIRECT),
        "where the stored grant returns, not where this client asked"
    );
    let section = app
        .account_page(id.clone())
        .and_then(|page| page.sign_in)
        .expect("a section");
    assert_eq!(section.browser.map(|out| out.attempt), Some(out.attempt));
    assert!(
        section.control.enabled && !section.control.running,
        "a route pressed while the browser waits starts it again"
    );
}

#[test]
fn a_return_from_another_request_ends_the_sign_in_saying_so() {
    let (app, id) = booted("browser-sign-in-stale");
    sign_in(&app, &id);
    let out = in_browser(&app);
    send(
        &app,
        AccountPageIntent::SignInBrowserEnded {
            account: id.clone(),
            attempt: out.attempt,
            end: SignInBrowserEnd::Redirected {
                callback_url: format!("{REDIRECT}?code=abc&state=another"),
            },
        },
    );
    over(&app);
    assert_eq!(said(&app, &id), failed(AccountChangeRefusal::StaleSignIn));
}

#[test]
fn a_fresh_press_supersedes_the_sign_in_out_and_its_return_is_dropped() {
    let (app, id) = booted("browser-sign-in-supersede");
    sign_in(&app, &id);
    let first = in_browser(&app);
    sign_in(&app, &id);
    let second = waits_for(&app, |out| {
        out.is_some_and(|out| {
            out.attempt != first.attempt
                && matches!(out.stage, BrowserSignInStage::InBrowser { .. })
        })
    })
    .expect("the second waits");
    send(
        &app,
        AccountPageIntent::SignInBrowserEnded {
            account: id.clone(),
            attempt: first.attempt,
            end: SignInBrowserEnd::Redirected {
                callback_url: format!("{REDIRECT}?code=abc&state=old"),
            },
        },
    );
    assert_eq!(
        app.browser_sign_in().map(|out| out.attempt),
        Some(second.attempt)
    );
    assert_eq!(said(&app, &id), None);
}

#[test]
fn a_cancel_ends_the_sign_in_and_says_nothing() {
    let (app, id) = booted("browser-sign-in-cancel");
    sign_in(&app, &id);
    let out = in_browser(&app);
    send(
        &app,
        AccountPageIntent::CancelSignIn {
            account: id.clone(),
            attempt: out.attempt,
        },
    );
    assert_eq!(app.browser_sign_in(), None);
    assert_eq!(said(&app, &id), None);
}

#[test]
fn a_browser_that_did_not_open_is_said() {
    let (app, id) = booted("browser-sign-in-not-opened");
    sign_in(&app, &id);
    let out = in_browser(&app);
    send(
        &app,
        AccountPageIntent::SignInBrowserEnded {
            account: id.clone(),
            attempt: out.attempt,
            end: SignInBrowserEnd::NotOpened,
        },
    );
    assert_eq!(app.browser_sign_in(), None);
    assert_eq!(
        said(&app, &id),
        failed(AccountChangeRefusal::BrowserNotOpened)
    );
}

#[test]
fn a_browser_left_without_returning_ends_the_sign_in_on_its_cap() {
    let (app, id) = booted("browser-sign-in-cap");
    app.account_work.browser.set_cap(Duration::from_millis(50));
    sign_in(&app, &id);
    in_browser(&app);
    over(&app);
    assert_eq!(said(&app, &id), failed(AccountChangeRefusal::NotFinished));
}

#[test]
fn the_notice_it_answers_runs_while_it_is_out_and_says_when_it_did_not_work() {
    let (app, id) = booted("browser-sign-in-notice");
    app.app
        .note_signin_expired(&engine_api::AccountId::try_from(id.as_str()).unwrap());
    let notice = |app: &MailcalApp| {
        app.notices()
            .notices
            .into_iter()
            .find(|notice| notice.account.as_deref() == Some(id.as_str()))
            .expect("the expired sign-in is said")
    };
    assert_eq!(notice(&app).action, Some(NoticeAction::SignInAgain));
    assert!(!notice(&app).running);

    sign_in(&app, &id);
    let out = in_browser(&app);
    assert!(notice(&app).running);
    send(
        &app,
        AccountPageIntent::SignInBrowserEnded {
            account: id.clone(),
            attempt: out.attempt,
            end: SignInBrowserEnd::Redirected {
                callback_url: format!("{REDIRECT}?code=abc&state=another"),
            },
        },
    );
    over(&app);
    let after = notice(&app);
    assert!(!after.running);
    assert!(after.remedy_failed);
}
