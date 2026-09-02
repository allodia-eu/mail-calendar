//! The one-time offer to become the OS's default mail app: when it may be put, and that the
//! answer survives a relaunch. The platform call itself is each client's; what is asserted here
//! is the decision every client shares (`docs/os-integration.md`). Fixtures: `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app_with_prefs};
use mailcal_viewmodel::{DefaultMailAppOutcome, DefaultMailAppSupport};

use super::Surface;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A preferences file of this test's own, so one test's stored answer cannot decide another's.
fn prefs(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-default-mail-app-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("preferences.toml")
}

#[tokio::test]
async fn a_build_that_can_act_offers_once_an_account_exists() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        prefs("offers"),
        &surfaces,
    );

    assert!(
        app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
    assert!(
        app.should_offer_default_mail_app(DefaultMailAppSupport::OpenSettings, Some(false))
            .await
    );
}

#[tokio::test]
async fn nothing_is_offered_before_the_first_account() {
    // On a first launch the app cannot send mail yet, so asking to be *the* mail app asks for a
    // commitment to something the person has not seen working.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(Vec::new(), prefs("no-account"), &surfaces);

    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
}

#[tokio::test]
async fn nothing_is_offered_where_the_build_can_do_nothing_about_it() {
    // Linux and Android reach here: no portal to ask through, and no role to request. A prompt
    // that cannot lead anywhere is worse than silence.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        prefs("unsupported"),
        &surfaces,
    );

    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::Unsupported, Some(false))
            .await
    );
    // Not even when the host cannot tell whether it is already the default.
    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::Unsupported, None)
            .await
    );
}

#[tokio::test]
async fn nothing_is_offered_when_we_are_already_the_default() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        prefs("already"),
        &surfaces,
    );

    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(true))
            .await
    );
}

#[tokio::test]
async fn an_unknown_default_is_treated_as_not_default() {
    // A Flatpak has no host application database to ask, so it reports `None`. Offering where
    // we need not is recoverable; staying quiet where we are not the default is the state this
    // whole feature exists to change.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        prefs("unknown"),
        &surfaces,
    );

    assert!(
        app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, None)
            .await
    );
}

#[tokio::test]
async fn the_offer_is_put_once_and_the_answer_survives_a_relaunch() {
    let path = prefs("once");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        path.clone(),
        &surfaces,
    );

    assert!(
        app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
    app.record_default_mail_app_offer(DefaultMailAppOutcome::Accepted)
        .await;

    // Spent for this session,
    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
    // and for every one after it: the answer is in the preferences file, not in memory.
    let relaunched = app_with_prefs(vec![account("acct", FakeProvider::new())], path, &surfaces);
    assert!(
        !relaunched
            .should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
    assert_eq!(relaunched.default_mail_app_offer(), Some(true));
}

#[tokio::test]
async fn a_declined_offer_is_never_put_again() {
    let path = prefs("declined");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(vec![account("acct", FakeProvider::new())], path, &surfaces);

    app.record_default_mail_app_offer(DefaultMailAppOutcome::Declined)
        .await;

    assert!(
        !app.should_offer_default_mail_app(DefaultMailAppSupport::SetDirectly, Some(false))
            .await
    );
    // Declined and accepted are both spent, and are still told apart, so Settings can say which.
    assert_eq!(app.default_mail_app_offer(), Some(false));
}

#[tokio::test]
async fn answering_the_offer_signals_the_settings_surface() {
    // The Settings row shows where things stand, so it has to re-pull when the answer changes.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_prefs(
        vec![account("acct", FakeProvider::new())],
        prefs("signals"),
        &surfaces,
    );
    surfaces.lock().unwrap().clear();

    app.record_default_mail_app_offer(DefaultMailAppOutcome::Accepted)
        .await;

    assert!(surfaces.lock().unwrap().contains(&Surface::Settings));
}
