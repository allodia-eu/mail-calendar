//! Sync-visibility tests for [`super::App`]: which per-account sync raises the "downloading Y of
//! X" bar, which only names itself in the background hint, and which says nothing at all. Adding
//! an account is an explicit, user-awaited download and must show the bar immediately; a
//! background check never raises it, but names its account once it actually downloads mail; an
//! empty poll / IDLE check says nothing on either surface. These run the real
//! `ProgressForwarder` over a real pass, which is what makes them the counterpart of
//! `sync_progress_tests.rs`, that one states the policy, this one proves the wiring reaches it.
//! The shared fixtures live in `tests_fakes.rs`. (The reset-shows-the-bar case lives with the loop
//! tests in `tests.rs`.)

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::{AccountId, EmailAddress, StreamTuning};
use fakes::{FakeProvider, account, app};
use mailcal_account::SyncDepth;

use super::{Account, sync_account::sync_account_providers};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Adding an account is an explicit, user-awaited first download, so its background first sync
/// raises the download bar immediately (unlike a poll / boot-reconnect refresh over
/// already-cached mail, which starts hidden). Regression guard for the report where a newly added
/// IMAP account downloaded with no visible progress; `sync_added_account` is the immediate
/// visible counterpart of `refresh_account`.
#[tokio::test]
async fn adding_an_account_shows_the_download_bar() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = Arc::new(app(vec![account("acct-1", FakeProvider::new())], &surfaces));
    assert!(!app.sync_progress().active);

    // The visible first sync runs on a task; catch the transient `active` peak like the reset test.
    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.sync_added_account(&AccountId::try_from("acct-1").unwrap())
                .await;
        }
    });
    let mut saw_bar = false;
    while !task.is_finished() {
        if app.sync_progress().active {
            saw_bar = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    task.await.unwrap();

    assert!(
        saw_bar,
        "adding an account must raise the download progress bar"
    );
    assert!(
        !app.sync_progress().active,
        "the bar hides once the first sync settles"
    );
}

/// A background per-account refresh never raises the bar; it takes a row of layout for work the
/// user did not start, but once it actually downloads mail it must name its account in the hint.
/// This is the catch-up case: a client behind by many messages should not look idle while
/// streamed rows are landing.
#[tokio::test]
async fn a_background_refresh_that_downloads_mail_names_its_account() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![fakes::message("m1", "a", "Catch-up")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));

    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.refresh_account(&AccountId::try_from("acct-1").unwrap())
                .await;
        }
    });
    after_commit.notified().await;
    let progress = app.sync_progress();
    assert!(
        !progress.active,
        "a background catch-up must not open the bar over the list"
    );
    let hint: Vec<_> = progress
        .accounts
        .iter()
        .map(|row| row.account_id.as_str())
        .collect();
    assert_eq!(
        hint,
        ["acct-1"],
        "a background catch-up must name its account once mail streams in"
    );
    finish.notify_one();
    task.await.unwrap();

    let settled = app.sync_progress();
    assert!(!settled.active);
    assert!(
        settled.accounts.is_empty(),
        "the hint clears once the catch-up sync settles"
    );
}

/// The counterpart: an empty background refresh (a poll tick / boot reconnect / IDLE check over
/// already-cached mail) must say nothing on either surface, so returning users never see progress
/// flash when there is nothing to download.
#[tokio::test]
async fn an_empty_background_refresh_says_nothing_at_all() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![fakes::message("m1", "a", "Already cached")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let id = AccountId::try_from("acct-1").unwrap();

    let first = tokio::spawn({
        let app = Arc::clone(&app);
        let id = id.clone();
        async move { app.sync_added_account(&id).await }
    });
    after_commit.notified().await;
    finish.notify_one();
    first.await.unwrap();
    assert!(!app.sync_progress().active);

    let second = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.refresh_account(&id).await }
    });
    after_commit.notified().await;
    let progress = app.sync_progress();
    assert!(
        !progress.active,
        "an empty background check must stay hidden while it is in flight"
    );
    assert!(
        progress.accounts.is_empty(),
        "and must not name its account either, nothing is arriving"
    );
    finish.notify_one();
    second.await.unwrap();
    assert!(!app.sync_progress().active);
}

/// Changing the depth while another sync owns the folder scope must not silently give up. The
/// settings path should wait/retry in the background so the wider/narrower window takes effect
/// without requiring a manual pull-to-refresh after the first sync settles.
#[tokio::test]
async fn sync_depth_change_waits_for_busy_scope_and_retries() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![fakes::message("m1", "a", "Already syncing")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let id = AccountId::try_from("acct-1").unwrap();

    let in_flight = tokio::spawn({
        let app = Arc::clone(&app);
        let id = id.clone();
        async move { app.refresh_account(&id).await }
    });
    after_commit.notified().await;

    let update = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.update_account_sync_depth("acct-1", 0).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !update.is_finished(),
        "a depth change must keep retrying instead of finishing while the scope is busy"
    );

    finish.notify_one();
    in_flight.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), after_commit.notified())
        .await
        .unwrap();
    finish.notify_one();
    tokio::time::timeout(Duration::from_secs(3), update)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(app.effective_sync_depth("acct-1"), SyncDepth::AllTime);
}

/// A reconnect catch-up after app restart must also wait through a busy folder scope. This covers
/// the boot race where IMAP watch startup syncs can grab folder leases before the account-level
/// resume pass starts, leaving the UI looking idle until the user manually pulls to refresh.
#[tokio::test]
async fn reconnect_catch_up_waits_for_busy_scope_and_retries() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![fakes::message("m1", "a", "Resumable catch-up")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let id = AccountId::try_from("acct-1").unwrap();

    let watch_startup_sync = tokio::spawn({
        let app = Arc::clone(&app);
        let id = id.clone();
        async move { app.refresh_account(&id).await }
    });
    after_commit.notified().await;

    let reconnect_catch_up = tokio::spawn({
        let app = Arc::clone(&app);
        let id = id.clone();
        async move { app.refresh_reconnected_account(&id).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !reconnect_catch_up.is_finished(),
        "reconnect catch-up must retry instead of finishing while a watch sync owns the scope"
    );

    finish.notify_one();
    watch_startup_sync.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), after_commit.notified())
        .await
        .unwrap();
    finish.notify_one();
    tokio::time::timeout(Duration::from_secs(3), reconnect_catch_up)
        .await
        .unwrap()
        .unwrap();
}

/// Interactive boot lists stored accounts as provider-less placeholders while the background
/// re-dial runs. A refresh over that placeholder should be an explicit skip, not a fake `Busy`
/// signal; otherwise the log points at scope contention when the real problem is simply "no live
/// providers yet".
#[tokio::test]
async fn providerless_placeholder_sync_is_skipped_not_busy() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(Vec::new(), &surfaces);
    let account = Account {
        id: AccountId::try_from("acct-1").unwrap(),
        providers: Vec::<FakeProvider>::new(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@acct-1.local"),
    };

    let progress = app.begin_sync_labeled(false, true, 0, "placeholder-test");
    let outcome = sync_account_providers(
        &app.engine,
        &account,
        StreamTuning::new(200, 1),
        &progress,
        0,
    )
    .await;
    app.end_sync(&progress);

    assert_eq!(outcome.reachable, None);
    assert_eq!(outcome.busy_scopes, 0);
}
