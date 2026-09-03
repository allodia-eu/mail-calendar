//! Tests for [`Intent::ActOnSelection`]: one action over the rows a user has selected.
//!
//! What is under test is the batching, not the individual write; those have their own tests in
//! `tests_settings`. A selection has to cost one sync per account rather than one per row, has
//! to route each row to its own account, has to expand a conversation the way the thread archive
//! does (a Sent copy stays in Sent), and has to put back exactly the row a provider refused.
//!
//! The shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_provider::MailEdit;
use fakes::{FakeProvider, account, app, message, msg, thread_ref, threaded};

use super::{BulkAction, Intent, RowRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The keys a batch of `MoveTo` edits moved, asserting they all went to `destination`.
fn moved_to(edits: &[MailEdit], destination: &str) -> Vec<String> {
    edits
        .iter()
        .map(|edit| match edit {
            MailEdit::MoveTo {
                target,
                destination: actual,
            } => {
                assert_eq!(actual.key().as_str(), destination);
                target.as_str().to_owned()
            }
            other => panic!("expected only MoveTo edits, got {other:?}"),
        })
        .collect()
}

/// Selected message rows, in the order given.
fn rows(account: &str, keys: &[&str]) -> Vec<RowRef> {
    keys.iter()
        .map(|key| RowRef::Message(msg(account, key)))
        .collect()
}

#[tokio::test]
async fn archiving_a_selection_moves_every_row_and_syncs_the_account_once() {
    // The reason this intent exists: dispatching `Archive` five times would sync the account
    // five times, because each single-row write re-syncs the account it touched.
    let provider = FakeProvider::with_archive(vec![
        message("m1", "a", "One"),
        message("m2", "a", "Two"),
        message("m3", "a", "Three"),
        message("m4", "a", "Four"),
        message("m5", "a", "Five"),
    ]);
    let edits = provider.edits();
    let syncs = provider.syncs();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    let before = syncs.load(Ordering::SeqCst);

    app.dispatch(Intent::ActOnSelection {
        rows: rows("acct-1", &["m1", "m2", "m3", "m4", "m5"]),
        action: BulkAction::Archive,
    })
    .await;

    let moved = moved_to(&edits.lock().unwrap(), "archive");
    assert_eq!(moved, vec!["m1", "m2", "m3", "m4", "m5"], "all five moved");
    assert_eq!(
        syncs.load(Ordering::SeqCst) - before,
        1,
        "five rows archived at once cost one account-wide sync, not five",
    );
    assert!(
        app.mailbox_list().rows.is_empty(),
        "the whole batch left the list",
    );
}

#[tokio::test]
async fn a_selection_spanning_accounts_moves_each_row_within_its_own_account() {
    // The unified list selects across accounts, and a provider key is unique only within one:
    // a batch that pooled the keys would archive whatever the other account calls "m1".
    let first = FakeProvider::with_archive(vec![message("m1", "a", "One")]);
    let second = FakeProvider::with_archive(vec![message("m2", "a", "Two")]);
    let (first_edits, second_edits) = (first.edits(), second.edits());
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", first), account("acct-2", second)],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: vec![
            RowRef::Message(msg("acct-1", "m1")),
            RowRef::Message(msg("acct-2", "m2")),
        ],
        action: BulkAction::Archive,
    })
    .await;

    assert_eq!(moved_to(&first_edits.lock().unwrap(), "archive"), ["m1"]);
    assert_eq!(moved_to(&second_edits.lock().unwrap(), "archive"), ["m2"]);
}

#[tokio::test]
async fn deleting_a_selection_moves_it_to_trash_rather_than_destroying_it() {
    let provider =
        FakeProvider::with_trash(vec![message("m1", "a", "One"), message("m2", "a", "Two")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: rows("acct-1", &["m1", "m2"]),
        action: BulkAction::Delete,
    })
    .await;

    assert_eq!(moved_to(&edits.lock().unwrap(), "trash"), ["m1", "m2"]);
}

#[tokio::test]
async fn permanently_deleting_a_selection_deletes_rather_than_moving() {
    let provider =
        FakeProvider::with_trash(vec![message("m1", "a", "One"), message("m2", "a", "Two")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: rows("acct-1", &["m1", "m2"]),
        action: BulkAction::PermanentlyDelete,
    })
    .await;

    let edits = edits.lock().unwrap();
    let deleted: Vec<&str> = edits
        .iter()
        .map(|edit| match edit {
            MailEdit::Delete { target } => target.as_str(),
            other => panic!("expected only Delete edits, got {other:?}"),
        })
        .collect();
    assert_eq!(deleted, ["m1", "m2"]);
}

#[tokio::test]
async fn marking_a_selection_read_edits_every_row_and_leaves_them_listed() {
    // Read/unread and flag/unflag take nothing out of a folder, so nothing is hidden and the
    // rows stay selectable: the user carries on acting on the same set.
    let provider = FakeProvider::with(vec![message("m1", "a", "One"), message("m2", "a", "Two")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: rows("acct-1", &["m1", "m2"]),
        action: BulkAction::MarkRead,
    })
    .await;

    let edits = edits.lock().unwrap();
    let targets: Vec<&str> = edits
        .iter()
        .map(|edit| match edit {
            MailEdit::SetKeywords { target, add, .. } => {
                assert!(!add.is_empty(), "read adds $seen rather than removing it");
                target.as_str()
            }
            other => panic!("expected only keyword edits, got {other:?}"),
        })
        .collect();
    assert_eq!(targets, ["m1", "m2"]);
    assert_eq!(
        app.mailbox_list().rows.len(),
        2,
        "a keyword edit hides no row",
    );
}

#[tokio::test]
async fn a_selected_conversation_archives_its_received_side_and_leaves_sent_alone() {
    // The rule the thread archive already follows, now reached through a selection: a threaded
    // row stands for its whole conversation, and a copy filed in Sent never leaves Sent.
    let provider = FakeProvider::with_sent_and_archive(vec![
        threaded("r1", "a", "Re: report", "t"),
        threaded("r2", "a", "Re: report", "t"),
        threaded("s1", "sent", "Re: report", "t"),
    ]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: vec![RowRef::Thread(thread_ref("acct-1", "t"))],
        action: BulkAction::Archive,
    })
    .await;

    let moved = moved_to(&edits.lock().unwrap(), "archive");
    assert_eq!(moved.len(), 2, "only the two received messages move");
    assert!(moved.contains(&"r1".to_owned()) && moved.contains(&"r2".to_owned()));
    assert!(
        !moved.contains(&"s1".to_owned()),
        "a Sent copy is never moved out of Sent",
    );
}

#[tokio::test]
async fn a_conversation_and_one_of_its_own_messages_is_written_once() {
    // Reachable from an expanded conversation: the header row and one of its sub-rows are both
    // selected. Writing the same key twice moves it and then edits a key the provider has
    // already retired, which reads back as a rejection over a move that in fact succeeded.
    let provider = FakeProvider::with_sent_and_archive(vec![
        threaded("r1", "a", "Re: report", "t"),
        threaded("r2", "a", "Re: report", "t"),
    ]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: vec![
            RowRef::Thread(thread_ref("acct-1", "t")),
            RowRef::Message(msg("acct-1", "r1")),
        ],
        action: BulkAction::Archive,
    })
    .await;

    let mut moved = moved_to(&edits.lock().unwrap(), "archive");
    moved.sort();
    assert_eq!(moved, ["r1", "r2"], "each message moved exactly once");
}

#[tokio::test]
async fn an_account_with_no_archive_folder_is_skipped_and_the_others_still_act() {
    // One account's shortfall must not swallow the rest of the batch: the unified list can
    // select across an account that advertises an Archive and one that does not.
    let with_archive = FakeProvider::with_archive(vec![message("m1", "a", "One")]);
    let without = FakeProvider::with(vec![message("m2", "a", "Two")]);
    let (moved_edits, untouched) = (with_archive.edits(), without.edits());
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account("acct-1", without), account("acct-2", with_archive)],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::ActOnSelection {
        rows: vec![
            RowRef::Message(msg("acct-1", "m2")),
            RowRef::Message(msg("acct-2", "m1")),
        ],
        action: BulkAction::Archive,
    })
    .await;

    assert!(
        untouched.lock().unwrap().is_empty(),
        "the account with no Archive folder wrote nothing",
    );
    assert_eq!(moved_to(&moved_edits.lock().unwrap(), "archive"), ["m1"]);
}

#[tokio::test]
async fn a_refused_write_brings_its_own_row_back_and_leaves_the_batch_hidden() {
    // A provider that refuses every write: each row's hide is undone individually, so a
    // rejection cannot leave a message hidden from a list it never left.
    let provider =
        FakeProvider::with_archive(vec![message("m1", "a", "One"), message("m2", "a", "Two")]);
    let refuse = provider.failure_switch();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    refuse.store(true, Ordering::SeqCst);
    app.dispatch(Intent::ActOnSelection {
        rows: rows("acct-1", &["m1", "m2"]),
        action: BulkAction::Archive,
    })
    .await;

    assert_eq!(
        app.mailbox_list().rows.len(),
        2,
        "neither row stayed hidden over a refused move",
    );
}
