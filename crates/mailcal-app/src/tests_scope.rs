//! What the mailbox list is **scoped to**: the unified inbox, one account, or one folder of one
//! account, and what moves it there.
//!
//! Split from `tests.rs` (which keeps the refresh/derive/notify loop and pagination) because
//! these are one rule with one owner: [`Scope`](crate::scope::Scope), which binds a folder key to
//! the account that owns it so the pair can never come apart (`docs/folder-pane.md`, rule 14).
//! On-demand folder sync lives here too; it is what *opening* a folder does.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use fakes::{
    FakeConnector, FakeProvider, FlakyConnector, ObservingConnector, account, app,
    app_with_connector, flat_subjects, message, open_folder,
};
use mailcal_viewmodel::SnapshotRow;

use super::{Intent, Surface};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn opening_an_unsynced_folder_downloads_it_on_demand() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // The connector serves an "archive" folder the eager bind never synced (its one
    // message lives in the `archive` mailbox).
    let connector = FakeConnector::new(vec![(
        "archive".to_owned(),
        vec![message("c1", "archive", "Archived report")],
    )]);
    let app = app_with_connector(
        vec![account("acct-1", FakeProvider::new())],
        connector,
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;

    // Selecting the never-synced folder connects a provider for it and streams it in, so
    // its message appears; without it, the folder view would be empty.
    app.dispatch(open_folder("acct-1", "archive")).await;
    let snapshot = app.mailbox_list();
    assert_eq!(snapshot.selected.as_deref(), Some("archive"));
    assert!(
        flat_subjects(&snapshot)
            .iter()
            .any(|s| s == "Archived report"),
        "the on-demand folder's message should be visible: {:?}",
        flat_subjects(&snapshot)
    );
    // The download reported progress.
    assert!(surfaces.lock().unwrap().contains(&Surface::SyncProgress));

    // Re-opening the folder does not reconnect it (it's already synced/attempted): the
    // connector would only ever be asked once. Leaving the folder is re-selecting its
    // account; there is no folder-less `SelectFolder` to dispatch.
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    app.dispatch(open_folder("acct-1", "archive")).await;
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Archived report")
    );
}

#[tokio::test]
async fn the_folder_is_on_screen_before_its_mail_is_downloaded() {
    // Opening a folder the eager bind skipped costs a provider connection and a download.
    // Awaiting that before publishing left the window on the folder the user had just left
    // for the length of a network round trip: the click looked like it had missed. The
    // scope is published first; the download's own pass raises the progress bar and
    // publishes again when mail lands (`docs/sync-progress.md`).
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = ObservingConnector::new(
        "archive",
        vec![message("c1", "archive", "Archived report")],
        &surfaces,
    );
    let published = connector.published_before_connect();
    let app = app_with_connector(
        vec![account("acct-1", FakeProvider::new())],
        connector,
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    surfaces.lock().unwrap().clear();

    app.dispatch(open_folder("acct-1", "archive")).await;

    // The connector is the network. By the time it was reached, the host had already been
    // handed a list: the assertion the old ordering fails.
    assert_eq!(
        *published.lock().unwrap(),
        Some(1),
        "the folder's snapshot must be published before the download begins"
    );
    // And the download's mail still arrives.
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Archived report")
    );
}

#[tokio::test]
async fn a_transient_folder_connect_failure_retries_on_the_next_open() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    // The connector fails its first connect attempt for "archive" (a network blip), then
    // succeeds on the next.
    let connector = FlakyConnector::new(
        "archive",
        vec![message("c1", "archive", "Archived report")],
        1,
    );
    let attempts = connector.attempts();
    let app = app_with_connector(
        vec![account("acct-1", FakeProvider::new())],
        connector,
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;

    // First open: the connect fails, so the folder shows empty for now…
    app.dispatch(open_folder("acct-1", "archive")).await;
    assert!(
        !flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Archived report"),
        "a transient connect failure leaves the folder empty for now"
    );

    // …but the failed attempt was NOT remembered, so re-opening retries and succeeds (the
    // bug was that the folder stayed empty until app restart). Leaving the folder is
    // re-selecting its account.
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    app.dispatch(open_folder("acct-1", "archive")).await;
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Archived report"),
        "re-opening after a transient failure re-attempts the connect: {:?}",
        flat_subjects(&app.mailbox_list())
    );
    assert_eq!(
        *attempts.lock().unwrap(),
        2,
        "the folder connect was retried rather than blocked for the session"
    );
}

#[tokio::test]
async fn unified_inbox_merges_accounts_and_selecting_one_filters() {
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

    // The unified all-inboxes view shows both accounts' inbox mail, each row tagged with
    // its account, and the switcher lists both accounts.
    let snapshot = app.mailbox_list();
    assert!(snapshot.selected_account.is_none());
    assert_eq!(snapshot.accounts.len(), 2);
    let tagged: Vec<(&str, &str)> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => (r.account.as_str(), r.subject.as_str()),
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    assert!(tagged.contains(&("work", "Work mail")));
    assert!(tagged.contains(&("home", "Home mail")));

    // Selecting one account filters to just its mail.
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    let work = app.mailbox_list();
    assert_eq!(work.selected_account.as_deref(), Some("work"));
    assert_eq!(work.rows.len(), 1);
    assert!(
        work.rows
            .iter()
            .all(|row| matches!(row, SnapshotRow::Flat(r) if r.account == "work"))
    );

    // Back to all inboxes.
    app.dispatch(Intent::SelectAccount(None)).await;
    assert!(app.mailbox_list().selected_account.is_none());
    assert_eq!(app.mailbox_list().rows.len(), 2);
}

#[tokio::test]
async fn a_folder_opens_in_the_account_it_names_whatever_was_selected() {
    // Both accounts have an `archive`, because every provider names its folders the same way;
    // which is the whole reason a folder key cannot be dispatched on its own. The pane shows
    // every account's tree at once (`docs/folder-pane.md`), so the key alone would be resolved
    // against whichever account happened to be selected, and from the unified list against none
    // at all: the click changed nothing and the list stayed on All Inboxes.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![
                    message("w1", "a", "Work inbox"),
                    message("w2", "archive", "Filed at work"),
                ]),
            ),
            account(
                "home",
                FakeProvider::with(vec![
                    message("h1", "a", "Home inbox"),
                    message("h2", "archive", "Filed at home"),
                ]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert!(
        app.mailbox_list().selected_account.is_none(),
        "opens unified"
    );

    // Straight from the unified list, with no account selected: the folder brings its own.
    app.dispatch(open_folder("home", "archive")).await;
    let home = app.mailbox_list();
    assert_eq!(home.selected_account.as_deref(), Some("home"));
    assert_eq!(home.selected.as_deref(), Some("archive"));
    assert_eq!(flat_subjects(&home), vec!["Filed at home"]);

    // And the same key, dispatched against the other account, opens the OTHER mailbox rather
    // than being resolved against the one already selected.
    app.dispatch(open_folder("work", "archive")).await;
    let work = app.mailbox_list();
    assert_eq!(work.selected_account.as_deref(), Some("work"));
    assert_eq!(flat_subjects(&work), vec!["Filed at work"]);
}

#[tokio::test]
async fn removing_the_selected_account_falls_back_to_the_unified_inbox() {
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
    assert_eq!(app.mailbox_list().selected_account.as_deref(), Some("work"));

    // Removing the *selected* account falls back to the unified "all inboxes".
    app.remove_account(&AccountId::try_from("work").unwrap())
        .await;
    let snapshot = app.mailbox_list();
    assert!(snapshot.selected_account.is_none());
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(snapshot.accounts[0].id, "home");
}
