//! Runtime/loop tests for [`super::App`]: the dispatch → sync → derive → snapshot → notify
//! cycle, the flat/threaded re-projection, pagination, and adding or removing an account. What
//! the list is *scoped to* (the unified inbox, an account, a folder) is `tests_scope.rs`. The
//! shared fixtures (FakeProvider, observer, helpers) live in `tests_fakes.rs` so each file stays
//! under the 500-line limit.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::AccountId;
use fakes::{FakeProvider, account, app, flat_subjects, message};
use mailcal_viewmodel::{MailboxListSnapshot, SnapshotRow, ViewMode};

use super::{Intent, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn refresh_mail_syncs_derives_snapshots_and_notifies() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    // Before any dispatch the snapshot is empty and the observer silent.
    assert_eq!(app.mailbox_list(), MailboxListSnapshot::default());
    assert!(surfaces.lock().unwrap().is_empty());

    app.dispatch(Intent::RefreshMail).await;

    // The whole loop ran end-to-end: the two synced inbox messages appear in the default
    // unified all-inboxes view (threaded by default, but two lone messages each form a
    // single-message conversation, so each still projects as a flat row), and the surface
    // fired once.
    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.mode, ViewMode::Threaded);
    assert!(snapshot.selected_account.is_none());
    let subjects = flat_subjects(&snapshot);
    assert!(
        subjects.iter().any(|s| s == "Quarterly report")
            && subjects.iter().any(|s| s == "Lunch plans")
    );
    // The refresh publishes once from the streamed commit, then once from the final
    // authoritative rebuild (plus sync-progress pulses, which have their own coverage).
    let mailbox_signals;
    {
        let signalled = surfaces.lock().unwrap();
        mailbox_signals = signalled
            .iter()
            .filter(|s| **s == Surface::MailboxList)
            .count();
        assert!(mailbox_signals >= 2);
        assert!(signalled.contains(&Surface::SyncProgress));
    }

    // Toggling to flat re-projects + signals the list again (still two rows: the two lone
    // messages, but now explicitly the flat view).
    app.dispatch(Intent::SetViewMode(ViewMode::Flat)).await;
    let flat = app.mailbox_list();
    assert_eq!(flat.mode, ViewMode::Flat);
    assert_eq!(flat.rows.len(), 2);
    assert_eq!(
        surfaces
            .lock()
            .unwrap()
            .iter()
            .filter(|s| **s == Surface::MailboxList)
            .count(),
        mailbox_signals + 1
    );
}

#[tokio::test]
async fn streamed_commit_updates_the_mailbox_list_before_sync_finishes() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![message("m-live", "a", "Live streamed row")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let account = AccountId::try_from("acct-1").unwrap();

    let sync = {
        let app = Arc::clone(&app);
        tokio::spawn(async move {
            app.sync_account(&account).await;
        })
    };
    tokio::time::timeout(Duration::from_secs(1), after_commit.notified())
        .await
        .expect("streamed commit should land before the gated stream finishes");

    assert!(
        flat_subjects(&app.mailbox_list()).contains(&"Live streamed row".to_owned()),
        "the commit delta should be visible before the sync future completes",
    );
    assert!(
        surfaces.lock().unwrap().contains(&Surface::MailboxList),
        "the live commit should signal the mailbox-list surface",
    );

    finish.notify_waiters();
    tokio::time::timeout(Duration::from_secs(1), sync)
        .await
        .expect("gated sync should finish")
        .expect("sync task should not panic");
}

/// A reset is an explicit, user-awaited full re-download, so the download progress bar must
/// show even though the pre-reset inbox is still painted (a routine refresh over already-
/// listed mail syncs silently). Regression guard for the missing-feedback report where reset
/// ran but showed no progress on macOS/Android.
#[tokio::test]
async fn reset_shows_the_download_bar_even_when_mail_is_already_listed() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // Gate the stream so the bar can be read at a point the re-download is provably still in
    // flight. Polling for a transient peak instead would sample a window that is a small
    // fraction of `reset`; most of its wall time is the cache clear and the closing VACUUM,
    // both with the bar down, and a sample that lands either side of it reads as "no bar".
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![message("m-1", "a", "Quarterly report")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));

    // Paint the inbox first: the list is non-empty afterwards, and the bar is idle once the
    // initial sync settles: so any bar the reset raises is the reset's, not left over.
    let paint = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(Intent::RefreshMail).await }
    });
    release(&after_commit, &finish).await;
    paint.await.unwrap();
    assert!(!app.mailbox_list().rows.is_empty());
    assert!(!app.sync_progress().active);

    let task = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.reset().await }
    });
    tokio::time::timeout(Duration::from_secs(5), after_commit.notified())
        .await
        .expect("the reset should reach the gated stream");
    assert!(
        app.sync_progress().active,
        "a reset must raise the download progress bar",
    );
    finish.notify_waiters();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("the released reset should finish")
        .unwrap();

    assert!(
        !app.sync_progress().active,
        "the bar hides once the reset settles"
    );
}

/// Waits for a [`FakeProvider::blocking`] stream to reach its gate, then lets it run on.
async fn release(after_commit: &tokio::sync::Notify, finish: &tokio::sync::Notify) {
    tokio::time::timeout(Duration::from_secs(5), after_commit.notified())
        .await
        .expect("the sync should reach the gated stream");
    finish.notify_waiters();
}

#[tokio::test]
async fn remove_account_drops_it_from_the_switcher_and_the_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![message("w1", "a", "Work mail")]),
            ),
            account(
                "home",
                FakeProvider::with(vec![message("h1", "a", "Home mail")]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(app.mailbox_list().accounts.len(), 2);

    // Removing an account leaves the switcher and takes its mail out of the unified list.
    app.remove_account(&AccountId::try_from("home").unwrap())
        .await;
    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(snapshot.accounts[0].id, "work");
    let subjects: Vec<&str> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.subject.as_str(),
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    assert_eq!(subjects, vec!["Work mail"]);
}

#[tokio::test]
async fn the_mailbox_list_paginates_and_show_more_grows_the_window() {
    use crate::PAGE;

    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // One account with more than two pages of inbox mail.
    let many: Vec<_> = (0..(PAGE * 2 + 30))
        .map(|i| message(&format!("m{i}"), "a", &format!("msg {i}")))
        .collect();
    let total = many.len();
    let app = app(vec![account("acct-1", FakeProvider::with(many))], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // The first screen is one page; `total` reports the whole set so the host can scroll.
    let first = app.mailbox_list();
    assert_eq!(first.rows.len(), PAGE);
    assert_eq!(first.total, total);

    // Each ShowMore grows the window by a page…
    app.dispatch(Intent::ShowMore).await;
    assert_eq!(app.mailbox_list().rows.len(), PAGE * 2);

    // …and a window past the end just returns every row (no over-run).
    app.dispatch(Intent::ShowMore).await;
    let full = app.mailbox_list();
    assert_eq!(full.rows.len(), total);
    assert_eq!(full.total, total);

    // A navigation resets the window to the first page (selecting the account here).
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    assert_eq!(app.mailbox_list().rows.len(), PAGE);

    // Grow again, then switching view mode also resets to the first page.
    app.dispatch(Intent::ShowMore).await;
    assert_eq!(app.mailbox_list().rows.len(), PAGE * 2);
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    // Each message is its own conversation here (no thread id): a lone message projects as a
    // flat row, so a page is still PAGE rows.
    assert_eq!(app.mailbox_list().rows.len(), PAGE);
    assert_eq!(app.mailbox_list().total, total);
}

#[tokio::test]
async fn search_keeps_the_selected_account_scope() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![message("w1", "a", "Work mail")]),
            ),
            account(
                "home",
                FakeProvider::with(vec![message("h1", "a", "Home mail")]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;

    // A search while an account is selected must keep the switcher on that account, not
    // report the unified "all inboxes" view (regardless of how many hits come back).
    app.dispatch(Intent::Search(Some("anything".to_owned())))
        .await;
    assert_eq!(app.mailbox_list().selected_account.as_deref(), Some("work"));
}

#[tokio::test]
async fn add_account_brings_a_second_account_into_the_unified_inbox() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![message("w1", "a", "Work mail")]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(app.mailbox_list().rows.len(), 1);

    app.add_account(account(
        "home",
        FakeProvider::with(vec![message("h1", "a", "Home mail")]),
    ))
    .await;

    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(flat_subjects(&snapshot).len(), 2);
}

#[tokio::test]
async fn re_adding_an_account_replaces_it_instead_of_duplicating() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![message("w1", "a", "First sync")]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    // Re-adding the same account id (e.g. after a credential change) reconnects it rather
    // than duplicating it in the switcher; its newly synced mail replaces the old.
    app.add_account(account(
        "work",
        FakeProvider::with(vec![message("w2", "a", "Reconnected")]),
    ))
    .await;

    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(flat_subjects(&snapshot), vec!["Reconnected"]);
}
