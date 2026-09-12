//! What reaches the list while a search is still being typed: rule 10 of `docs/search.md`.
//!
//! Searches run concurrently (intents are spawned, not queued) and finish out of order, so a
//! prefix matching half the mailbox can land after the query the user finished typing. Both
//! halves an in-flight rebuild carries, its query and the generation it read, are handed in
//! directly here, so these are the race itself rather than an attempt to win a scheduling
//! coin-toss.

use std::sync::{Arc, Mutex};

use super::{
    Intent, dated,
    fakes::{FakeProvider, account, app, flat_subjects},
};

/// **The regression test for the search that answers a question the user finished asking.**
///
/// Typing runs a search per keystroke, concurrently, and they finish in an order nobody
/// controls: a two-letter prefix matches half the mailbox and takes a second, the full query
/// takes a tenth. The older one must not repaint the list when it lands.
///
/// Both halves an in-flight rebuild carries (its query and the generation it read) are handed
/// in directly, so this is the race itself rather than an attempt to win a scheduling coin-toss.
#[tokio::test]
async fn a_search_a_newer_keystroke_superseded_never_reaches_the_list() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![
                dated("w1", "a", "Report on the merger", 4),
                dated("w2", "a", "Repairs to the roof", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    // The keystroke before last: "Rep" matches both messages, and its rebuild is still running.
    let superseded = app.search_state().generation;

    // The last keystroke lands and answers first.
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;
    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        ["Report on the merger"],
        "the query the user finished typing is what the list shows",
    );
    let signals = surfaces.lock().unwrap().len();

    // Now "Rep" finishes, a beat late and twice as broad.
    app.rebuild_snapshot_for(Some("Rep"), superseded).await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        ["Report on the merger"],
        "a search the user has typed past must not win the list by finishing last",
    );
    assert_eq!(
        surfaces.lock().unwrap().len(),
        signals,
        "a superseded rebuild must not publish at all: a host that reconciles rows would \
         repaint the list to the same content and scroll it",
    );
}

/// The guard drops what is stale, not what is slow: a rebuild still answering the current
/// search publishes normally, however long it took.
#[tokio::test]
async fn a_search_still_current_when_it_finishes_publishes() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![
                dated("w1", "a", "Report on the merger", 4),
                dated("w2", "a", "Repairs to the roof", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    // Nothing has been typed since, so this rebuild's generation is still the current one.
    let current = app.search_state().generation;
    app.rebuild_snapshot_for(Some("Rep"), current).await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        ["Report on the merger", "Repairs to the roof"],
        "a rebuild nothing superseded must reach the list",
    );
}
