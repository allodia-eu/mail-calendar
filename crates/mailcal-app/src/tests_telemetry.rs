//! Consented-analytics tests for [`super::App`].
//!
//! Two things are being proved here, and they are the whole feature:
//!
//! 1. **The consent gate holds.** Nothing is minted, built, or sent until the user opts in;
//!    declining sends nothing; withdrawing stops the stream, clears the id, and asks the backend to
//!    erase it. This is an ePrivacy Art. 5(3) obligation, not a preference; see
//!    `docs/analytics.md`.
//! 2. **The payload cannot carry content.** A search query, an address, a subject, or a folder name
//!    must not reach the wire even when the app is driven through the intents that carry them. The
//!    type system is supposed to make that impossible; these tests check that it actually does,
//!    against a real dispatch rather than a hand-built event.
//!
//! The shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use engine_api::Engine;
use fakes::{FakeProvider, account};
use mailcal_account::{MessageGrouping, SwipeAction, load_preferences, save_preferences};

use super::{
    Account, App, AppObserver, Batch, DeviceClass, DeviceInfo, Intent, NOTICE_VERSION, Platform,
    Surface, Telemetry, TelemetrySink, TimeZoneInit,
};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

// The payload/no-content half of these tests. A CHILD module, not a sibling, so it can see the
// fixtures above without widening `FakeProvider` beyond this test tree.
#[path = "tests_telemetry_payload.rs"]
mod payload;

/// These tests assert on what the *sink* received, not on which surfaces were signalled, so the
/// observer just absorbs.
#[derive(Debug)]
struct SilentObserver;

impl AppObserver for SilentObserver {
    fn surface_changed(&self, _surface: Surface) {}
}

/// A sink that records every batch and erase instead of sending it. The stand-in for the network,
/// so a test can assert on the exact bytes that *would* have left the device.
#[derive(Debug, Default)]
struct RecordingSink {
    batches: Mutex<Vec<Batch>>,
    erased: Mutex<Vec<String>>,
}

impl TelemetrySink for RecordingSink {
    fn send(&self, batch: Batch) {
        self.batches.lock().unwrap().push(batch);
    }

    fn erase(&self, install_id: String) {
        self.erased.lock().unwrap().push(install_id);
    }
}

/// A sink handle a test keeps a reference to, plus the `Box` the app takes.
#[derive(Debug)]
struct SinkHandle(Arc<RecordingSink>);

impl TelemetrySink for SinkHandle {
    fn send(&self, batch: Batch) {
        self.0.send(batch);
    }

    fn erase(&self, install_id: String) {
        self.0.erase(install_id);
    }
}

/// The device facts a host would report. Deliberately *raw*: a full OS version and a regional
/// locale: so the tests prove the core reduces them rather than trusting the client to.
fn device() -> DeviceInfo {
    DeviceInfo {
        platform: Platform::Macos,
        os_version: "15.4.1".to_owned(),
        device_class: DeviceClass::MacLaptop,
        app_version: "1.4.0".to_owned(),
        locale: "nl-NL".to_owned(),
    }
}

/// An app with analytics **wired but not consented**: the state every real first launch is in.
/// Persists consent to `prefs_path` so the relaunch cases can reload it.
fn app_with_analytics(
    accounts: Vec<Account<FakeProvider>>,
    prefs_path: std::path::PathBuf,
) -> (App<FakeProvider>, Arc<RecordingSink>) {
    let sink = Arc::new(RecordingSink::default());
    let app = App::new(
        Engine::open_in_memory().unwrap(),
        accounts,
        TimeZoneInit {
            device_zone: engine_api::TimeZoneId::utc(),
            prefs_path: Some(prefs_path.clone()),
        },
        None,
        std::sync::Arc::new(SilentObserver),
        Telemetry::new(
            Some(prefs_path),
            device(),
            Box::new(SinkHandle(Arc::clone(&sink))),
        ),
    );
    (app, sink)
}

/// A scratch preferences path, cleaned first so a rerun starts unasked.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mailcal-telemetry-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("preferences.toml")
}

/// One account, so a dispatch has something to act on. `account` gives it the identity
/// `me@acct-1.local`: an address the payload must never carry, which the content test relies on.
fn one_account() -> Vec<Account<FakeProvider>> {
    vec![account("acct-1", FakeProvider::new())]
}

#[tokio::test]
async fn a_background_calendar_refresh_is_not_feature_adoption() {
    let path = scratch("background-calendar");
    let (app, sink) = app_with_analytics(one_account(), path);
    app.set_analytics_consent(true);
    sink.batches.lock().unwrap().clear();

    app.refresh_calendar_in_background().await;
    assert!(
        sink.batches.lock().unwrap().is_empty(),
        "timer-driven calendar sync must not report that the user opened the calendar"
    );

    app.dispatch(Intent::RefreshCalendar).await;
    let wire = serde_json::to_string(&*sink.batches.lock().unwrap()).unwrap();
    assert!(
        wire.contains("calendar"),
        "the explicit calendar action remains adoption"
    );
}

// ---------------------------------------------------------------------------------------------
// The consent gate.
// ---------------------------------------------------------------------------------------------

/// The one that matters. A first launch has been asked nothing, so it sends nothing; even though
/// the sink is wired and the app is driven through the intents that would otherwise emit. Under
/// ePrivacy Art. 5(3), *writing the identifier* is itself the act that needs consent, so there is
/// also no install id on disk to write.
#[tokio::test]
async fn nothing_is_sent_and_no_id_is_minted_before_consent() {
    let path = scratch("unasked");
    let (app, sink) = app_with_analytics(one_account(), path.clone());

    assert!(!app.analytics_consent().asked, "a first launch is unasked");
    assert!(!app.analytics_consent().enabled);

    app.report_app_opened();
    app.dispatch(Intent::Search(Some("secret".to_owned())))
        .await;
    app.dispatch(Intent::RefreshCalendar).await;

    assert!(
        sink.batches.lock().unwrap().is_empty(),
        "an unasked user must produce no telemetry at all"
    );
    assert_eq!(
        load_preferences(&path).analytics_install_id,
        None,
        "no consent, no identifier written to the device"
    );
}

/// Declining is recorded (so we never ask again) and sends nothing.
#[tokio::test]
async fn declining_is_remembered_and_sends_nothing() {
    let path = scratch("declined");
    let (app, sink) = app_with_analytics(one_account(), path.clone());

    app.set_analytics_consent(false);

    let consent = app.analytics_consent();
    assert!(consent.asked, "we asked, so we do not ask again");
    assert!(!consent.enabled);

    app.report_app_opened();
    app.dispatch(Intent::RefreshCalendar).await;
    assert!(sink.batches.lock().unwrap().is_empty());
    assert_eq!(load_preferences(&path).analytics_install_id, None);
}

/// Opting in mints the id and starts the stream, and counts the session in which consent was
/// given, rather than leaving it invisible until the next launch.
#[tokio::test]
async fn opting_in_mints_an_id_and_starts_sending() {
    let path = scratch("consented");
    let (app, sink) = app_with_analytics(one_account(), path.clone());

    app.set_analytics_consent(true);

    let consent = app.analytics_consent();
    assert!(consent.asked && consent.enabled);

    let stored = load_preferences(&path);
    let id = stored
        .analytics_install_id
        .expect("an id is minted at consent");
    assert!(!id.is_empty());
    assert_eq!(stored.analytics_notice_version, Some(NOTICE_VERSION));
    assert!(
        stored.analytics_consented_at.is_some(),
        "GDPR Art. 7(1): we must be able to demonstrate *when* consent was given"
    );

    let batches = sink.batches.lock().unwrap();
    assert!(
        !batches.is_empty(),
        "consenting counts the session it was given in"
    );
    assert!(batches.iter().all(|batch| batch.install_id == id));
    assert!(
        batches
            .iter()
            .any(|batch| { batch.events.iter().any(|event| event.name == "app_opened") })
    );
}

/// Withdrawal stops the stream, clears the id from the device, and asks the backend to erase
/// everything held under it (GDPR Art. 17), which is only possible *because* the id is stable.
#[tokio::test]
async fn withdrawing_stops_the_stream_clears_the_id_and_erases_the_backend() {
    let path = scratch("withdrawn");
    let (app, sink) = app_with_analytics(one_account(), path.clone());

    app.set_analytics_consent(true);
    let id = load_preferences(&path).analytics_install_id.unwrap();
    let sent_before = sink.batches.lock().unwrap().len();
    assert!(sent_before > 0);

    app.set_analytics_consent(false);

    assert_eq!(
        sink.erased.lock().unwrap().as_slice(),
        &[id],
        "the backend is asked to erase the id that was in force"
    );
    assert_eq!(
        load_preferences(&path).analytics_install_id,
        None,
        "the identifier is gone from the device"
    );
    assert_eq!(
        load_preferences(&path).analytics_consented_at,
        None,
        "there is no live consent left to demonstrate"
    );

    app.report_app_opened();
    app.dispatch(Intent::RefreshCalendar).await;
    assert_eq!(
        sink.batches.lock().unwrap().len(),
        sent_before,
        "nothing more is sent after withdrawal"
    );
}

/// A consent given against an **older** notice reads back as unasked, so widening what we send
/// re-asks instead of quietly inheriting a yes that was given for less. Until the user answers
/// again, nothing is sent.
#[tokio::test]
async fn a_stale_notice_version_re_asks_and_stops_the_stream() {
    let path = scratch("stale-notice");
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    save_preferences(
        &path,
        &mailcal_account::Preferences {
            analytics_consent: Some(true),
            analytics_install_id: Some("an-old-id".to_owned()),
            // Consent was given against a notice that no longer describes what we send.
            analytics_notice_version: Some(NOTICE_VERSION - 1),
            ..Default::default()
        },
    )
    .unwrap();

    let (app, sink) = app_with_analytics(one_account(), path);
    let consent = app.analytics_consent();
    assert!(!consent.asked, "a stale consent must be re-asked");
    assert!(!consent.enabled);

    app.report_app_opened();
    assert!(
        sink.batches.lock().unwrap().is_empty(),
        "a stale consent licenses nothing"
    );
}

/// A *decline*, unlike a consent, survives a notice bump: we asked once and were told no. Bumping
/// the notice must not become a way to re-prompt someone who already refused.
#[tokio::test]
async fn a_decline_survives_a_notice_bump() {
    let path = scratch("declined-old-notice");
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    save_preferences(
        &path,
        &mailcal_account::Preferences {
            analytics_consent: Some(false),
            analytics_notice_version: Some(NOTICE_VERSION - 1),
            ..Default::default()
        },
    )
    .unwrap();

    let (app, _) = app_with_analytics(one_account(), path);
    let consent = app.analytics_consent();
    assert!(consent.asked, "we do not re-prompt someone who said no");
    assert!(!consent.enabled);
}

/// Consent is a persisted decision: it survives a relaunch, id and all.
#[tokio::test]
async fn consent_and_its_id_survive_a_relaunch() {
    let path = scratch("relaunch");
    let (first, _) = app_with_analytics(one_account(), path.clone());
    first.set_analytics_consent(true);
    let id = load_preferences(&path).analytics_install_id.unwrap();
    drop(first);

    let (second, sink) = app_with_analytics(one_account(), path);
    assert!(second.analytics_consent().enabled);
    second.report_app_opened();
    assert!(
        sink.batches
            .lock()
            .unwrap()
            .iter()
            .all(|batch| batch.install_id == id),
        "the same install is recognised across launches, that is what retention needs"
    );
}

/// The consent write is read-modify-write, so the sibling preferences in the same file survive.
#[tokio::test]
async fn writing_consent_preserves_sibling_preferences() {
    let path = scratch("siblings");
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    save_preferences(
        &path,
        &mailcal_account::Preferences {
            display_timezone: Some("Europe/Amsterdam".to_owned()),
            message_grouping: MessageGrouping::Flat,
            swipe_left: SwipeAction::Archive,
            ..Default::default()
        },
    )
    .unwrap();

    let (app, _) = app_with_analytics(one_account(), path.clone());
    app.set_analytics_consent(true);

    let stored = load_preferences(&path);
    assert_eq!(stored.display_timezone.as_deref(), Some("Europe/Amsterdam"));
    assert_eq!(stored.message_grouping, MessageGrouping::Flat);
    assert_eq!(stored.swipe_left, SwipeAction::Archive);
    assert!(stored.analytics_install_id.is_some());
}
