//! Threaded-mode projection: grouping by `(account, thread_id)`, latest-in-scope
//! summarization/ordering, cross-folder conversation membership, cross-folder dedup, and the
//! lone-message-as-a-flat-row rule.

use super::*;

#[test]
fn threaded_windows_to_the_limit_and_reports_thread_total() {
    // Three distinct threads (each a real two-message conversation, so each stays a thread row;
    // a lone message would project as a flat row), a window of one: `total` counts threads (3),
    // and the single returned row is the newest thread.
    let messages = vec![
        at("a", &message("m1", "Oldest", 5, Some("t1"))),
        at("a", &message("m1b", "Oldest earlier", 0, Some("t1"))),
        at("a", &message("m2", "Middle", 20, Some("t2"))),
        at("a", &message("m2b", "Middle earlier", 15, Some("t2"))),
        at("a", &message("m3", "Newest", 40, Some("t3"))),
        at("a", &message("m3b", "Newest earlier", 35, Some("t3"))),
    ];
    let snapshot = build(
        &messages,
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        1,
    );
    assert_eq!(snapshot.total, 3, "total counts threads, not the window");
    assert_eq!(snapshot.rows.len(), 1);
    let SnapshotRow::Thread(first) = &snapshot.rows[0] else {
        panic!("expected a thread row");
    };
    assert_eq!(
        first.subject, "Newest",
        "the window leads with the newest thread"
    );
}

#[test]
fn threaded_row_reflects_attachments_anywhere_in_the_conversation() {
    let latest = message("m1", "Latest", 30, Some("t"));
    let mut older_attached = message("m2", "Older", 0, Some("t"));
    older_attached.has_attachment = true;
    let snapshot = build(
        &[at("a", &latest), at("a", &older_attached)],
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    let SnapshotRow::Thread(thread) = &snapshot.rows[0] else {
        panic!("expected thread row");
    };
    assert!(thread.has_attachment);
    assert_eq!(thread.latest_key, "m1");
}

#[test]
fn threaded_groups_by_thread_and_summarizes_latest() {
    let messages = vec![
        at("a", &message("m1", "Re: report", 0, Some("t"))),
        at("a", &message("m2", "Re: report (reply)", 30, Some("t"))),
        at("a", &message("m3", "Lunch", 15, None)),
    ];
    let snapshot = build(
        &messages,
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    assert_eq!(snapshot.mode, ViewMode::Threaded);
    assert_eq!(snapshot.rows.len(), 2);

    let SnapshotRow::Thread(first) = &snapshot.rows[0] else {
        panic!("expected a thread row");
    };
    assert_eq!(first.thread_id, "t");
    assert_eq!(first.account, "a");
    assert_eq!(first.message_count, 2);
    assert_eq!(first.subject, "Re: report (reply)"); // the latest message's subject
    assert_eq!(first.latest_key, "m2"); // opening the thread opens its newest message
}

#[test]
fn threaded_keeps_two_accounts_threads_separate() {
    // Same thread id in two accounts must not merge into one conversation. Each account's
    // conversation carries two messages so it stays a thread row (a lone message would project
    // as a flat row): so dropping the account from the group key would merge all four into one
    // thread, which this guards against.
    let messages = vec![
        at("work", &message("m1", "Shared id (work)", 10, Some("t"))),
        at(
            "work",
            &message("m1b", "Shared id (work) reply", 12, Some("t")),
        ),
        at("home", &message("m2", "Shared id (home)", 20, Some("t"))),
        at(
            "home",
            &message("m2b", "Shared id (home) reply", 22, Some("t")),
        ),
    ];
    let snapshot = build(
        &messages,
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    assert_eq!(snapshot.rows.len(), 2);
}

#[test]
fn threaded_conversation_includes_out_of_folder_members_newest_first() {
    // Viewed from the Inbox: an inbound message (in scope) and the owner's own Sent reply
    // (out of scope, filed only in Sent). The thread is shown because the inbound message is
    // in scope, and it carries BOTH messages; newest first, the Sent reply flagged outgoing.
    let inbound = at("a", &message("m1", "Re: report", 0, Some("t")));
    let sent_reply = member(
        "a",
        &message("m2", "Re: report (reply)", 30, Some("t")),
        true,
    );
    let snapshot = build(
        &[inbound, sent_reply],
        &[],
        &[],
        vec![],
        Some("a"),
        Some("inbox"),
        ViewMode::Threaded,
        ALL,
    );
    assert_eq!(snapshot.rows.len(), 1, "one conversation touches the inbox");
    let SnapshotRow::Thread(thread) = &snapshot.rows[0] else {
        panic!("expected a thread row");
    };
    assert_eq!(
        thread.message_count, 2,
        "both the inbound and the sent reply count"
    );
    // The summary + `latest_key` are the latest IN-SCOPE (received) message, NOT the newer Sent
    // reply that lives only in Sent: so a follow-up you send doesn't relabel the Inbox row "from
    // me" or steal what opens. The full chain is still carried in `messages`.
    assert_eq!(
        thread.latest_key, "m1",
        "the representative is the latest in-scope message, not the out-of-folder Sent reply"
    );
    assert_eq!(
        thread.subject, "Re: report",
        "the summary is the received message's subject"
    );
    let messages = &thread.messages;
    assert_eq!(
        messages.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(),
        vec!["m2", "m1"],
        "conversation is newest first"
    );
    assert!(
        messages[0].outgoing,
        "the sent reply carries the Sent badge"
    );
    assert!(!messages[1].outgoing, "the inbound message does not");
}

#[test]
fn threaded_orders_by_latest_in_scope_not_by_a_sent_reply() {
    // The reported bug: replying to an old thread must not jump it to the top of the Inbox. A
    // thread's Inbox position is its latest IN-SCOPE (received) message; the owner's own Sent reply
    // (filed in Sent, out of scope here) rides in the conversation for reference but doesn't
    // reorder the folder. Thread A: received long ago (0), the owner's reply just now (59, out of
    // scope: the newest message overall). Thread B: a newer received exchange (latest in scope
    // 50). B must lead A in the Inbox despite A's newer Sent reply.
    let a_recv = at("acct", &message("a1", "Old thread", 0, Some("ta")));
    let a_reply = member(
        "acct",
        &message("a2", "Old thread (my reply)", 59, Some("ta")),
        true,
    );
    let b_recv = at("acct", &message("b1", "Newer thread", 50, Some("tb")));
    let b_recv_earlier = at(
        "acct",
        &message("b0", "Newer thread (earlier)", 40, Some("tb")),
    );
    let snapshot = build(
        &[a_recv, a_reply, b_recv, b_recv_earlier],
        &[],
        &[],
        vec![],
        Some("acct"),
        Some("inbox"),
        ViewMode::Threaded,
        ALL,
    );
    let subjects: Vec<&str> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Thread(thread) => thread.subject.as_str(),
            SnapshotRow::Flat(flat) => flat.subject.as_str(),
        })
        .collect();
    assert_eq!(
        subjects,
        vec!["Newer thread", "Old thread"],
        "the just-replied-to old thread stays below the newer received thread"
    );
}

#[test]
fn threaded_hides_conversations_with_no_in_scope_member() {
    // A thread living entirely outside the shown folder (e.g. a Sent-only exchange seen from
    // the Inbox) must not appear: only threads that touch the scope are listed.
    let out = member("a", &message("m1", "Sent only", 0, Some("t")), true);
    let snapshot = build(
        &[out],
        &[],
        &[],
        vec![],
        Some("a"),
        Some("inbox"),
        ViewMode::Threaded,
        ALL,
    );
    assert!(snapshot.rows.is_empty());
    assert_eq!(snapshot.total, 0);
}

#[test]
fn threaded_dedups_cross_folder_copies_by_message_id() {
    // The same message filed in two folders (one in scope, one not); e.g. an Inbox copy and
    // a Gmail "All Mail" copy; shares a Message-ID and collapses to one conversation entry,
    // keeping the in-scope copy's key so the row opens the message the user is looking at. A
    // reply keeps the conversation multi-message (so it stays a thread rather than projecting as
    // a flat row): the dedup is exercised on its two copies of the original.
    let inbox_copy = at(
        "a",
        &with_message_id(message("inbox-key", "Hello", 10, Some("t")), "same@id"),
    );
    let archive_copy = member(
        "a",
        &with_message_id(message("archive-key", "Hello", 10, Some("t")), "same@id"),
        false,
    );
    let reply = at("a", &message("reply-key", "Re: Hello", 20, Some("t")));
    let snapshot = build(
        &[archive_copy, inbox_copy, reply],
        &[],
        &[],
        vec![],
        Some("a"),
        Some("inbox"),
        ViewMode::Threaded,
        ALL,
    );
    let messages = thread_messages(&snapshot);
    assert_eq!(
        messages.len(),
        2,
        "the two copies of the original collapse to one entry, plus the reply"
    );
    // Newest first: the reply, then the single deduped original (kept as its in-scope copy).
    assert_eq!(messages[0].key, "reply-key");
    assert_eq!(messages[1].key, "inbox-key", "the in-scope copy is kept");
}

#[test]
fn threaded_projects_a_lone_message_as_a_flat_row() {
    // A "conversation" of a single message is not a thread: in threaded mode it projects as a
    // flat row (no expand affordance, opens directly), so a host never shows a one-message
    // expandable thread. Only real multi-message conversations render as threads.
    let messages = vec![
        at("a", &message("m1", "Re: report", 0, Some("t"))),
        at("a", &message("m2", "Re: report (reply)", 30, Some("t"))),
        at("a", &message("solo", "Lunch?", 15, None)),
    ];
    let snapshot = build(
        &messages,
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Threaded,
        ALL,
    );
    assert_eq!(
        snapshot.rows.len(),
        2,
        "the two-message thread plus the lone message"
    );
    // Newest activity first: the reply (30) leads as a thread, the lone message (15) is flat.
    assert!(
        matches!(snapshot.rows[0], SnapshotRow::Thread(_)),
        "the real conversation stays a thread"
    );
    let SnapshotRow::Flat(flat) = &snapshot.rows[1] else {
        panic!("expected the lone message to be a flat row, not a one-message thread");
    };
    assert_eq!(flat.subject, "Lunch?");
    assert_eq!(flat.key, "solo");
}

#[test]
fn threaded_order_is_stable_across_rebuilds_when_threads_tie_on_latest_activity() {
    // Regression: threads are grouped in a `HashMap`, whose iteration order varies per rebuild.
    // Ordering only by latest activity left threads that tie (same instant) in that arbitrary
    // order, so every snapshot reshuffled the list. Hosts reconcile rows by id, so the reshuffle
    // became a move storm: on WinUI each moved row's container is destroyed and re-created, and
    // the whole list visibly blanked every time a new mail arrived.
    let tied = || {
        // Six two-message conversations, every one landing on the same minute.
        (1..=6)
            .flat_map(|n| {
                let thread = format!("t{n}");
                [
                    at("a", &message(&format!("m{n}"), "Latest", 30, Some(&thread))),
                    at(
                        "a",
                        &message(&format!("m{n}b"), "Earlier", 10, Some(&thread)),
                    ),
                ]
            })
            .collect::<Vec<_>>()
    };
    let ids = |messages: &[AccountMessage]| {
        build(
            messages,
            &[],
            &[],
            vec![],
            None,
            None,
            ViewMode::Threaded,
            ALL,
        )
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Thread(t) => t.thread_id.clone(),
            SnapshotRow::Flat(f) => f.key.clone(),
        })
        .collect::<Vec<_>>()
    };

    let first = ids(&tied());
    assert_eq!(first.len(), 6, "every tied conversation is a thread row");
    // A fresh `HashMap` per build reseeds its iteration order, so a non-total order would
    // reshuffle here; repeat enough that a lucky match can't hide it.
    for _ in 0..16 {
        assert_eq!(
            ids(&tied()),
            first,
            "tied threads must project in the same order on every rebuild",
        );
    }
    // And that order is the tiebreak, not chance.
    assert_eq!(first, ["t1", "t2", "t3", "t4", "t5", "t6"]);
}

#[test]
fn conversation_members_order_is_independent_of_the_supplied_order_when_instants_tie() {
    // The same total-order rule governs a conversation's expanded sub-rows: two messages sharing an
    // instant must not swap places (nor swap which copy survives dedup) just because the group's
    // members were collected in a different order.
    let newest = at("a", &message("m3", "Newest", 40, Some("t")));
    let tie_early = at("a", &message("m1", "Tied", 10, Some("t")));
    let tie_late = at("a", &message("m2", "Tied too", 10, Some("t")));
    let member_keys = |messages: Vec<AccountMessage>| {
        let snapshot = build(
            &messages,
            &[],
            &[],
            vec![],
            None,
            None,
            ViewMode::Threaded,
            ALL,
        );
        let SnapshotRow::Thread(thread) = &snapshot.rows[0] else {
            panic!("expected a thread row");
        };
        thread
            .messages
            .iter()
            .map(|m| m.key.clone())
            .collect::<Vec<_>>()
    };

    let forward = member_keys(vec![newest.clone(), tie_early.clone(), tie_late.clone()]);
    let reversed = member_keys(vec![tie_late, tie_early, newest]);
    assert_eq!(
        forward, reversed,
        "tied conversation members must order the same whatever order they arrive in",
    );
    // Newest first, then the `(account, key)` tiebreak among the two tied members.
    assert_eq!(forward, ["m3", "m1", "m2"]);
}
