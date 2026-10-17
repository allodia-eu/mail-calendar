//! What reaches the list when the selection moves while a rebuild is still running.
//!
//! Rebuilds run concurrently (a click's beside the one a sync's commit started) and finish out
//! of order. The selection an in-flight rebuild read is handed in directly here, so these are the
//! race itself rather than an attempt to win a scheduling coin-toss.

use std::sync::{Arc, Mutex};

use mailcal_viewmodel::ViewMode;

use super::{
    Intent, Surface,
    fakes::{FakeProvider, account, app, flat_subjects, message, open_folder},
};
use crate::App;

fn one_account(surfaces: &Arc<Mutex<Vec<Surface>>>) -> App<FakeProvider> {
    app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![
                message("i1", "inbox", "In the inbox"),
                message("d1", "drafts", "A draft"),
            ]),
        )],
        surfaces,
    )
}

/// **The regression test for the folder that snapped back.** A draft saved, its follow-up sync
/// began rebuilding the Inbox the user was in, and the user opened Drafts: the Drafts list was
/// published, then the Inbox rebuild finished a moment later and put the Inbox back.
#[tokio::test]
async fn a_rebuild_for_the_folder_just_left_never_reaches_the_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = one_account(&surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(open_folder("acct-1", "inbox")).await;

    // The sync's rebuild reads the selection: the Inbox.
    let left = app.scope.lock().unwrap().clone();
    let mode = *app.view_mode.lock().unwrap();
    let generation = app.search_state().generation;

    // The user opens Drafts, and its rebuild answers first.
    app.dispatch(open_folder("acct-1", "drafts")).await;
    assert_eq!(flat_subjects(&app.mailbox_list()), ["A draft"]);
    let signals = surfaces.lock().unwrap().len();

    // Now the Inbox rebuild finishes.
    app.rebuild_snapshot_in(&left, mode, None, generation).await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        ["A draft"],
        "a rebuild for a folder the user has left must not win the list by finishing last",
    );
    assert_eq!(
        surfaces.lock().unwrap().len(),
        signals,
        "a superseded rebuild must not publish at all: the host would redraw the list it already shows",
    );
}

/// The same for the grouping: a rebuild begun in one view mode does not overwrite the other.
#[tokio::test]
async fn a_rebuild_for_the_view_mode_just_left_never_reaches_the_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = one_account(&surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetViewMode(ViewMode::Threaded)).await;
    let scope = app.scope.lock().unwrap().clone();
    let generation = app.search_state().generation;

    app.dispatch(Intent::SetViewMode(ViewMode::Flat)).await;
    let signals = surfaces.lock().unwrap().len();

    app.rebuild_snapshot_in(&scope, ViewMode::Threaded, None, generation)
        .await;

    assert_eq!(
        surfaces.lock().unwrap().len(),
        signals,
        "a rebuild in the mode the user switched away from must not publish",
    );
}

/// The guard drops what is stale, not what is slow: a rebuild still answering the current
/// selection publishes, however long it took.
#[tokio::test]
async fn a_rebuild_for_the_current_folder_publishes() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = one_account(&surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(open_folder("acct-1", "drafts")).await;
    let current = app.scope.lock().unwrap().clone();
    let mode = *app.view_mode.lock().unwrap();
    let signals = surfaces.lock().unwrap().len();

    app.rebuild_snapshot_in(&current, mode, None, app.search_state().generation)
        .await;

    assert_eq!(flat_subjects(&app.mailbox_list()), ["A draft"]);
    assert!(
        surfaces.lock().unwrap().len() > signals,
        "a rebuild for the folder on screen publishes"
    );
}
