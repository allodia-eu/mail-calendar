//! Tests for Settings → Advanced's reset: it shows the download bar, runs one at a time, and says
//! what it came to.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::Engine;
use mailcal_viewmodel::{ResetOutcome, ResetStatus};

use crate::{
    Intent, Surface,
    tests_fakes::{FakeProvider, account, app, app_on, message},
};

/// Waits for a [`FakeProvider::blocking`] stream to reach its gate, then lets it run on.
async fn release(after_commit: &tokio::sync::Notify, finish: &tokio::sync::Notify) {
    tokio::time::timeout(Duration::from_secs(5), after_commit.notified())
        .await
        .expect("the sync should reach the gated stream");
    finish.notify_waiters();
}

fn settings_signals(surfaces: &Mutex<Vec<Surface>>) -> usize {
    surfaces
        .lock()
        .unwrap()
        .iter()
        .filter(|surface| **surface == Surface::Settings)
        .count()
}

/// A reset is an explicit, awaited full re-download, so the download bar shows although the
/// inbox from before it is still painted (a routine refresh over listed mail syncs silently).
/// Meanwhile the reset is running, a second press starts nothing, and once the mail is back
/// it is done.
#[tokio::test]
async fn a_reset_runs_once_shows_the_download_bar_and_says_it_is_done() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // Gate the stream so the reset can be read while the re-download is provably in flight.
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![message("m-1", "a", "Quarterly report")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));

    // Paint the inbox first, so any bar the reset raises is the reset's.
    let paint = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(Intent::RefreshMail).await }
    });
    release(&after_commit, &finish).await;
    paint.await.unwrap();
    assert_ne!(app.mailbox_list().rows, []);
    assert!(!app.sync_progress().active);
    assert_eq!(app.reset_status(), ResetStatus::default());
    let before = settings_signals(&surfaces);

    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(Intent::ResetDatabase).await }
    });
    tokio::time::timeout(Duration::from_secs(5), after_commit.notified())
        .await
        .expect("the reset should reach the gated stream");
    assert!(
        app.sync_progress().active,
        "a reset raises the download bar"
    );
    assert_eq!(
        app.reset_status(),
        ResetStatus {
            running: true,
            outcome: None
        }
    );
    assert_eq!(
        settings_signals(&surfaces),
        before + 1,
        "Settings hears it start"
    );

    // Asked again while it runs: dropped at once, without a second clear or a second sync.
    tokio::time::timeout(Duration::from_secs(1), app.dispatch(Intent::ResetDatabase))
        .await
        .expect("a second reset returns at once");
    assert!(app.reset_status().running);

    finish.notify_waiters();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("the released reset should finish")
        .unwrap();

    assert!(!app.sync_progress().active, "the bar hides once it settles");
    assert_eq!(
        app.reset_status(),
        ResetStatus {
            running: false,
            outcome: Some(ResetOutcome::Done)
        }
    );
    assert_eq!(settings_signals(&surfaces), before + 2, "and hears it end");
    assert_ne!(app.mailbox_list().rows, [], "the mail is back");
}

/// A store the engine cannot write ends the reset as failed, and nothing is downloaded again
/// over a cache that was never cleared.
#[tokio::test]
async fn a_reset_the_store_refuses_is_failed_and_downloads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mailcal.sqlite");
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_on(
        Engine::open(&path).unwrap(),
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m-1", "a", "Quarterly report")]),
        )],
        &surfaces,
    );

    // Another process holding the database for writing: the engine's write waits for it, then
    // gives up.
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    app.dispatch(Intent::ResetDatabase).await;
    holder.execute_batch("ROLLBACK;").unwrap();

    assert_eq!(
        app.reset_status(),
        ResetStatus {
            running: false,
            outcome: Some(ResetOutcome::Failed)
        }
    );
    assert_eq!(app.mailbox_list().rows, [], "no re-download ran");
    assert_eq!(settings_signals(&surfaces), 2);
}
