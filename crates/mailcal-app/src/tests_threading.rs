//! Threading tests for [`super::App`]: a conversation the engine had to *derive* (an IMAP-shaped
//! provider that assigns no thread ids of its own) must keep re-grouping as new mail lands, not
//! only on the first sync.
//!
//! Derivation has to keep considering a message whose thread id it derived itself. Treating a
//! derived id as a provider-assigned one excludes the message from every later pass, so each reply
//! arriving afterwards becomes its own one-message conversation and the mailbox list shows the
//! thread torn into separate rows until a full resync. The engine carries that distinction in
//! `Message.thread` + `ThreadProvenance`; this pins the behaviour the client actually shows.
//!
//! The shared fixtures live in `tests_fakes.rs`.

use std::sync::{Arc, Mutex};

use engine_core::{
    ids::AccountId,
    mail::{Keyword, SystemKeyword},
};
use fakes::{FakeProvider, account, app, unthreaded};
use mailcal_viewmodel::{MailboxListSnapshot, SnapshotRow};

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A reply that arrives in a **later** sync than the message it answers must join that
/// conversation, leaving the mailbox one threaded row: not a fresh one-message thread beside it.
#[tokio::test]
async fn a_reply_arriving_after_the_first_sync_joins_its_derived_thread() {
    // An IMAP-shaped inbox: no provider thread ids, only RFC 5322 headers, so the engine derives
    // the conversation itself.
    let provider = FakeProvider::with(vec![unthreaded(
        "m1",
        "a",
        "Quarterly report",
        "root@example.com",
        &[],
    )]);
    let late = provider.late_delivery();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);

    // First sync: one message, and derivation assigns it a thread id.
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        app.mailbox_list().rows.len(),
        1,
        "the one message is listed"
    );

    // The reply lands after that first pass: the exact case the bug broke.
    late.lock().unwrap().push(unthreaded(
        "m2",
        "a",
        "Re: Quarterly report",
        "reply@example.com",
        &["root@example.com"],
    ));
    app.dispatch(Intent::RefreshMail).await;

    // Both messages are in one conversation: a single row, and it is a thread of two.
    let rows = app.mailbox_list().rows;
    assert_eq!(
        rows.len(),
        1,
        "the reply must join the existing conversation, not open a second row",
    );
    match &rows[0] {
        SnapshotRow::Thread(thread) => {
            assert_eq!(thread.message_count, 2, "both messages are on the thread");
        }
        SnapshotRow::Flat(row) => panic!("expected a threaded row, got a flat one: {row:?}"),
    }
}

/// The mailbox list must show `expected` messages on its one conversation.
fn assert_one_thread_of(list: &MailboxListSnapshot, expected: u32, when: &str) {
    assert_eq!(list.rows.len(), 1, "{when}: the list must stay one row");
    match &list.rows[0] {
        SnapshotRow::Thread(thread) => assert_eq!(
            thread.message_count, expected,
            "{when}: the conversation must keep all its messages"
        ),
        SnapshotRow::Flat(row) => panic!("{when}: expected a threaded row, got {row:?}"),
    }
}

/// Opening a message marks it read, and the server echoes that back as a flag change: the
/// same message, re-mapped, carrying its keywords and, because a provider without
/// server-side threading has none to send: no thread id.
///
/// The list the user is looking at must not tear apart while that lands. Thread derivation
/// runs *after* the sync, so a snapshot taken mid-pass is the one that shows the damage:
/// the message drops out of its conversation into a row of its own, and jumps back a moment
/// later. This parks the stream immediately after the flag-only chunk commits: the exact
/// instant the live mailbox snapshot is rebuilt from it, and asserts the conversation is
/// still whole.
#[tokio::test]
async fn a_flag_only_echo_mid_sync_never_un_groups_the_open_message() {
    let root = unthreaded("m1", "a", "Quarterly report", "root@example.com", &[]);
    let reply = unthreaded(
        "m2",
        "a",
        "Re: Quarterly report",
        "reply@example.com",
        &["root@example.com"],
    );
    let (provider, after_commit, finish) = FakeProvider::blocking(vec![root.clone(), reply]);
    let late = provider.late_delivery();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let id = AccountId::try_from("acct-1").unwrap();

    // First sync: both messages land and derivation groups them.
    let first = tokio::spawn({
        let (app, id) = (Arc::clone(&app), id.clone());
        async move { app.sync_added_account(&id).await }
    });
    after_commit.notified().await;
    finish.notify_one();
    first.await.unwrap();
    assert_one_thread_of(&app.mailbox_list(), 2, "after the first sync");

    // The message the user opened comes back marked read, and with no thread; what an IMAP
    // server sends and what the engine must restore from what it already holds.
    let mut seen = root;
    seen.keywords.insert(Keyword::system(SystemKeyword::Seen));
    assert!(seen.thread.is_none(), "the echo carries no thread id");
    late.lock().unwrap().push(seen);

    let refresh = tokio::spawn({
        let (app, id) = (Arc::clone(&app), id.clone());
        async move { app.refresh_account(&id).await }
    });
    after_commit.notified().await;
    assert_one_thread_of(
        &app.mailbox_list(),
        2,
        "while the flag-only chunk is landing",
    );
    finish.notify_one();
    refresh.await.unwrap();

    // And it is still whole once the pass settles; derivation repaired nothing, because
    // nothing was broken.
    assert_one_thread_of(&app.mailbox_list(), 2, "after the pass settles");
}
