use std::{
    cell::RefCell,
    sync::{atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};

use mailcal_bindings::{MailcalError, NextcloudSignInPoll};

use super::{FlightEnd, Grant, wait};

/// Answers `answers` in order, one per poll.
fn polls(
    answers: Vec<Result<NextcloudSignInPoll, MailcalError>>,
) -> impl Fn(&str) -> Result<NextcloudSignInPoll, MailcalError> {
    let answers = RefCell::new(answers.into_iter());
    move |pending: &str| {
        assert_eq!(pending, "pending");
        answers.borrow_mut().next().expect("asked once too often")
    }
}

#[test]
fn a_grant_ends_the_wait_after_the_polls_that_found_nobody_yet() {
    let poll = polls(vec![
        Ok(NextcloudSignInPoll::Waiting),
        Ok(NextcloudSignInPoll::Waiting),
        Ok(NextcloudSignInPoll::Granted {
            grant: "granted".to_owned(),
        }),
    ]);
    let (_wake, woken) = mpsc::channel();
    let end = wait(
        &AtomicBool::new(false),
        "pending",
        Duration::from_millis(1),
        &woken,
        poll,
    );
    assert!(matches!(end, FlightEnd::Granted(Grant(grant)) if grant == "granted"));
}

#[test]
fn a_window_coming_back_asks_at_once_rather_than_at_the_next_interval() {
    let poll = polls(vec![
        Ok(NextcloudSignInPoll::Waiting),
        Ok(NextcloudSignInPoll::Expired),
    ]);
    let (wake, woken) = mpsc::channel();
    wake.send(()).unwrap();
    let started = Instant::now();
    let end = wait(
        &AtomicBool::new(false),
        "pending",
        Duration::from_secs(60),
        &woken,
        poll,
    );
    assert!(matches!(end, FlightEnd::Expired));
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn a_retired_attempt_asks_nothing_more() {
    let poll = polls(Vec::new());
    let (_wake, woken) = mpsc::channel();
    let end = wait(
        &AtomicBool::new(true),
        "pending",
        Duration::from_millis(1),
        &woken,
        poll,
    );
    assert!(matches!(end, FlightEnd::Cancelled));
}

#[test]
fn an_unreadable_grant_ends_the_sign_in_and_says_it_failed() {
    let poll = polls(vec![Err(MailcalError::Connect("garbled".to_owned()))]);
    let (_wake, woken) = mpsc::channel();
    let end = wait(
        &AtomicBool::new(false),
        "pending",
        Duration::from_millis(1),
        &woken,
        poll,
    );
    assert_eq!(
        end.said().as_deref(),
        Some(crate::l10n::setup_imap_signin_failed())
    );
}

#[test]
fn an_expiry_says_so_and_a_cancel_says_nothing() {
    assert_eq!(
        FlightEnd::Expired.said().as_deref(),
        Some(crate::l10n::account_signin_timeout())
    );
    assert_eq!(FlightEnd::Cancelled.said(), None);
}

#[test]
fn a_grant_is_never_printed() {
    assert_eq!(
        format!("{:?}", Grant("app-password".to_owned())),
        "Grant(..)"
    );
}
