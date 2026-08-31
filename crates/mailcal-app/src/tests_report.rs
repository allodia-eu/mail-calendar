//! Tests for reporting a message to its provider: the mechanism behind mark-as-spam.
//!
//! The thing worth proving is invisible from the row: filing a message under Junk and
//! *reporting* it look identical to the list, and only one of them trains the filter that
//! decides where the next message lands. So these assert on what the **provider received**,
//! not on where the message ended up.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_api::{Provider, ReportVerdict};
use fakes::{FakeProvider, account, app, message, msg};

use crate::MailActionError;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// What the provider recorded: the reports it received, and the edits it received. The two
/// together are the assertion: a report that also moved the message would file it twice.
type Recorded = (
    Arc<Mutex<Vec<engine_api::MessageReport>>>,
    Arc<Mutex<Vec<engine_api::MailEdit>>>,
);

/// An app with one account whose provider can report and has somewhere to file junk.
fn reporting_app() -> (crate::App<FakeProvider>, Recorded) {
    let provider = FakeProvider::with(vec![message("m1", "a", "Cheap watches")]).with_junk_folder();
    let reports = provider.reports();
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    (app, (reports, edits))
}

#[tokio::test]
async fn marking_spam_reports_it_rather_than_only_moving_it() {
    let (app, (reports, edits)) = reporting_app();
    app.dispatch(crate::Intent::RefreshMail).await;

    app.act_spam(&msg("acct-1", "m1")).await.unwrap();

    let reports = reports.lock().unwrap();
    let report = reports.first().expect("the provider received a report");
    assert_eq!(report.verdict, ReportVerdict::Junk);
    assert_eq!(report.target.as_str(), "m1");
    assert_eq!(
        report.destination.key().as_str(),
        "junk",
        "the caller resolves the destination, as it does for a move",
    );
    assert!(
        edits.lock().unwrap().is_empty(),
        "a report files the message itself; sending a move as well would file it twice",
    );
}

#[tokio::test]
async fn marking_not_spam_reports_the_correction_back_to_the_inbox() {
    let (app, (reports, _)) = reporting_app();
    app.dispatch(crate::Intent::RefreshMail).await;

    app.act_not_spam(&msg("acct-1", "m1")).await.unwrap();

    let reports = reports.lock().unwrap();
    let report = reports.first().expect("the provider received a report");
    assert_eq!(report.verdict, ReportVerdict::NotJunk);
    assert_eq!(
        report.destination.key().as_str(),
        "a",
        "not-junk files it back in the Inbox, not in Junk",
    );
}

#[tokio::test]
async fn a_provider_that_cannot_report_still_files_the_message() {
    // The dev fixtures and the showcase engine advertise no reporting. The user asked for the
    // message to be moved; a provider that cannot be told is no reason to leave it in place.
    let provider = FakeProvider::with(vec![message("m1", "a", "Cheap watches")])
        .with_junk_folder()
        .without_reporting();
    let reports = provider.reports();
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;

    app.act_spam(&msg("acct-1", "m1")).await.unwrap();

    assert!(
        reports.lock().unwrap().is_empty(),
        "nothing should be reported to a provider that advertises no reporting",
    );
    assert_eq!(
        edits.lock().unwrap().len(),
        1,
        "it falls back to filing the message under Junk itself",
    );
}

#[tokio::test]
async fn an_account_with_no_junk_folder_says_so_rather_than_reporting_nowhere() {
    let provider = FakeProvider::with(vec![message("m1", "a", "Cheap watches")]);
    let reports = provider.reports();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;

    assert_eq!(
        app.act_spam(&msg("acct-1", "m1")).await,
        Err(MailActionError::NoTargetFolder),
    );
    assert!(reports.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_refused_report_surfaces_as_rejected_not_as_silent_success() {
    let provider = FakeProvider::with(vec![message("m1", "a", "Cheap watches")]).with_junk_folder();
    let down = provider.failure_switch();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(crate::Intent::RefreshMail).await;
    down.store(true, Ordering::SeqCst);

    assert_eq!(
        app.act_spam(&msg("acct-1", "m1")).await,
        Err(MailActionError::Rejected),
    );
}

#[tokio::test]
async fn a_transport_without_a_phishing_verdict_refuses_it() {
    // Gmail's label set has no phishing member, so the capability is what a caller must read.
    // Asking anyway is a hard error rather than a near-enough filing under spam.
    let provider = FakeProvider::with(vec![message("m1", "a", "Your account is suspended")])
        .with_junk_folder()
        .without_phishing_report();
    let caps = provider.connection_info().capabilities;
    let controls = caps.mail_report().expect("it can still report junk");

    assert!(controls.verdicts.allows(ReportVerdict::Junk));
    assert!(controls.verdicts.allows(ReportVerdict::NotJunk));
    assert!(
        !controls.verdicts.allows(ReportVerdict::Phishing),
        "a client builds its Report menu from this, so the gap has to be visible",
    );
}
