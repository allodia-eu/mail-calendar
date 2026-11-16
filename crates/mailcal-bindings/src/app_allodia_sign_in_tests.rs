//! The Allodia account's card through `dispatch`: the sign-in held as the core's one browser
//! sign-in, its Cancel, what a sign-in that did not complete says, and the sign-out run off the
//! caller's thread. Nothing here reaches the account service: a sign-in is put in the browser by
//! the slot itself, and the one started by `dispatch` needs a build without the service.

use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use crate::{
    AccountChangeRefusal, AccountPageIntent, AllodiaCard, AllodiaCardState, AllodiaFailure,
    AllodiaIntent, AllodiaOnboarding, BrowserSignInStage, Intent, LogLevel, MailcalApp,
    SignInBrowserEnd, Surface,
    account_page::browser::{Began, Pending, Start, Target},
    allodia::ACCOUNT_ID,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

const REDIRECT: &str = "mailcal.test://account-oauth";

fn booted(name: &str) -> (Arc<MailcalApp>, mpsc::Receiver<Surface>) {
    let (tx, rx) = mpsc::channel();
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        Vec::new(),
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::new(
            RecordingCredentialStore::default(),
        ))),
    )
    .expect("a core boots with no account");
    (app, rx)
}

fn send(app: &MailcalApp, intent: AllodiaIntent) {
    app.dispatch(Intent::Allodia { intent });
}

/// Puts an Allodia sign-in in the browser, as the service's answer would: answers its attempt.
fn in_browser(app: &MailcalApp, create: bool) -> u64 {
    let browser = &app.account_work.browser;
    let Start::Begin { attempt, .. } =
        browser.start(ACCOUNT_ID, Target::Allodia { create }, &[], false)
    else {
        panic!("nothing else is out");
    };
    let began = Began {
        pending: Pending::Allodia("{}".to_owned()),
        url: "https://accounts.test/authorize".to_owned(),
        redirect_uri: Some(REDIRECT.to_owned()),
        unreached: Vec::new(),
    };
    browser.began(attempt, began).expect("the attempt is out");
    app.signal_sign_in();
    attempt
}

fn until(app: &MailcalApp, done: impl Fn(&AllodiaCard) -> bool) -> AllodiaCard {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let card = app.allodia_card();
        if done(&card) {
            return card;
        }
        assert!(
            Instant::now() < deadline,
            "the card never got there: {card:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// How many `Surface::Allodia` signals came since the last look.
fn moved(rx: &mpsc::Receiver<Surface>) -> usize {
    std::thread::sleep(Duration::from_millis(100));
    rx.try_iter()
        .filter(|surface| matches!(surface, Surface::Allodia))
        .count()
}

#[test]
fn nobody_signed_in_is_offered_both_ways_in() {
    let (app, _rx) = booted("allodia-card-offered");
    let card = app.allodia_card();
    assert_eq!(card.state, AllodiaCardState::SignedOut);
    assert_eq!(card.onboarding, AllodiaOnboarding::Offering);
    assert!(card.browser.is_none());
    assert!(card.sign_in.enabled && card.create.enabled && card.sign_out.enabled);
    assert!(card.failure.is_none() && card.end_session.is_none());
}

#[test]
fn a_sign_in_in_the_browser_is_the_one_the_client_opens_and_holds_sign_out() {
    let (app, rx) = booted("allodia-card-browser");
    app.account_work.browser.set_opening(Duration::ZERO);
    let attempt = in_browser(&app, true);
    assert!(moved(&rx) > 0);
    let out = app.browser_sign_in().expect("the driver opens it");
    assert_eq!(out.attempt, attempt);
    assert!(matches!(
        out.stage,
        BrowserSignInStage::InBrowser { redirect_uri: Some(ref uri), .. } if uri == REDIRECT
    ));
    let card = app.allodia_card();
    assert_eq!(card.browser, Some(out));
    assert_eq!(
        card.onboarding,
        AllodiaOnboarding::SigningIn { escape: false }
    );
    // Once the browser is up, a fresh press starts it again, as an account's does.
    assert!(card.create.enabled && !card.create.running);
    assert!(
        !card.sign_out.enabled,
        "nobody signs out of a sign-in still out"
    );
}

#[test]
fn the_first_run_card_draws_its_way_back_once_the_sign_in_has_run_long_enough() {
    let (app, rx) = booted("allodia-card-escape");
    app.account_work
        .browser
        .set_escape_after(Duration::from_millis(50));
    let attempt = in_browser(&app, false);
    moved(&rx);
    // The core started no timer for an attempt it did not begin, so this asks it directly.
    MailcalApp::allodia_began(&app, attempt);
    let card = until(&app, |card| {
        card.onboarding == AllodiaOnboarding::SigningIn { escape: true }
    });
    assert!(card.browser.is_some());
    assert!(moved(&rx) > 0, "the card is told when its way back appears");
}

#[test]
fn a_cancelled_sign_in_says_nothing() {
    let (app, rx) = booted("allodia-card-cancel");
    let attempt = in_browser(&app, false);
    moved(&rx);
    send(&app, AllodiaIntent::CancelSignIn { attempt });
    let card = app.allodia_card();
    assert!(card.browser.is_none() && app.browser_sign_in().is_none());
    assert!(card.failure.is_none());
    assert_eq!(card.onboarding, AllodiaOnboarding::Offering);
    assert!(moved(&rx) > 0);
}

#[test]
fn a_cancel_names_its_attempt_and_leaves_any_other_sign_in_alone() {
    let (app, _rx) = booted("allodia-card-cancel-other");
    let attempt = in_browser(&app, false);
    send(
        &app,
        AllodiaIntent::CancelSignIn {
            attempt: attempt + 1,
        },
    );
    assert!(app.allodia_card().browser.is_some());
}

#[test]
fn a_browser_that_did_not_open_is_said_on_the_card() {
    let (app, _rx) = booted("allodia-card-not-opened");
    let attempt = in_browser(&app, false);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInBrowserEnded {
            account: ACCOUNT_ID.to_owned(),
            attempt,
            end: SignInBrowserEnd::NotOpened,
        },
    });
    let card = until(&app, |card| card.failure.is_some());
    assert_eq!(
        card.failure,
        Some(AllodiaFailure::SignIn {
            refusal: AccountChangeRefusal::BrowserNotOpened
        })
    );
    assert!(card.browser.is_none());
}

#[test]
fn a_redirect_the_service_will_not_take_is_said_on_the_card_and_stores_nothing() {
    let (app, _rx) = booted("allodia-card-refused");
    let attempt = in_browser(&app, false);
    app.dispatch(Intent::AccountPage {
        intent: AccountPageIntent::SignInBrowserEnded {
            account: ACCOUNT_ID.to_owned(),
            attempt,
            end: SignInBrowserEnd::Redirected {
                callback_url: format!("{REDIRECT}?code=c&state=s"),
            },
        },
    });
    let card = until(&app, |card| card.failure.is_some());
    assert!(matches!(card.failure, Some(AllodiaFailure::SignIn { .. })));
    assert_eq!(card.state, AllodiaCardState::SignedOut);
    assert!(card.browser.is_none());
    // Completed on a thread of its own: no account's page waited on it.
    assert!(!app.account_work.of("").0.busy);
}

#[test]
fn another_sign_in_replacing_it_moves_the_card() {
    let (app, rx) = booted("allodia-card-superseded");
    in_browser(&app, false);
    moved(&rx);
    app.account_work
        .browser
        .start("someone", Target::Account, &[], false);
    app.signal_sign_in();
    assert!(app.allodia_card().browser.is_none());
    assert!(moved(&rx) > 0);
}

#[test]
fn an_accounts_sign_in_moving_does_not_move_the_card() {
    let (app, rx) = booted("allodia-card-other-signals");
    app.account_work
        .browser
        .start("someone", Target::Account, &[], false);
    app.signal_sign_in();
    moved(&rx);
    app.signal_sign_in();
    assert_eq!(moved(&rx), 0);
}

#[test]
fn signing_out_runs_off_the_callers_thread_and_ends_with_nothing_to_say() {
    let (app, rx) = booted("allodia-card-sign-out");
    send(&app, AllodiaIntent::SignOut);
    let card = until(&app, |card| !card.sign_out.running);
    assert_eq!(card.state, AllodiaCardState::SignedOut);
    assert!(card.failure.is_none());
    // Nobody was signed in, so the service has no session to end.
    assert!(card.end_session.is_none());
    assert!(
        moved(&rx) >= 2,
        "the card is told as the sign-out starts and as it ends"
    );
}

#[test]
fn sign_out_is_held_while_a_sign_in_is_out() {
    let (app, _rx) = booted("allodia-card-sign-out-held");
    in_browser(&app, false);
    send(&app, AllodiaIntent::SignOut);
    std::thread::sleep(Duration::from_millis(100));
    assert!(!app.allodia_card().sign_out.running);
    assert!(app.allodia_card().browser.is_some());
}

/// A build without the service refuses the sign-in where it would start, and says so; it never
/// reaches the network.
#[cfg(not(feature = "allodia-license"))]
#[test]
fn a_build_without_the_service_says_the_sign_in_did_not_start() {
    let (app, _rx) = booted("allodia-card-unavailable");
    send(
        &app,
        AllodiaIntent::SignIn {
            create: false,
            redirect_uri: REDIRECT.to_owned(),
        },
    );
    let card = until(&app, |card| card.failure.is_some());
    assert!(card.browser.is_none());
}

#[test]
fn a_sign_in_with_nowhere_to_return_is_a_browser_that_did_not_open() {
    let (app, _rx) = booted("allodia-card-no-redirect");
    send(
        &app,
        AllodiaIntent::SignIn {
            create: false,
            redirect_uri: String::new(),
        },
    );
    let card = until(&app, |card| card.failure.is_some());
    assert_eq!(
        card.failure,
        Some(AllodiaFailure::SignIn {
            refusal: AccountChangeRefusal::BrowserNotOpened
        })
    );
}
