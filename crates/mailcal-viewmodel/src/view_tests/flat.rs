//! Flat-mode projection: newest-first ordering, the unified-inbox merge, windowing, and the
//! per-message row fields (date, flagged, attachment, in-scope filtering).

use super::*;

#[test]
fn flat_orders_newest_first() {
    let messages = vec![
        at("a", &message("m1", "Older", 0, None)),
        at("a", &message("m2", "Newer", 30, None)),
    ];
    let snapshot = build(&messages, &[], &[], vec![], None, None, ViewMode::Flat, ALL);
    assert_eq!(snapshot.mode, ViewMode::Flat);
    assert_eq!(flat_subjects(&snapshot), vec!["Newer", "Older"]);
}

#[test]
fn unified_inbox_merges_and_tags_accounts_newest_first() {
    // Two accounts' inbox messages interleave by date, each row tagged with its account.
    let messages = vec![
        at("work", &message("m1", "Work older", 0, None)),
        at("home", &message("m2", "Home newer", 30, None)),
        at("work", &message("m3", "Work newest", 45, None)),
    ];
    let snapshot = build(&messages, &[], &[], vec![], None, None, ViewMode::Flat, ALL);
    let rows: Vec<(&str, &str)> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => (r.account.as_str(), r.subject.as_str()),
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("work", "Work newest"),
            ("home", "Home newer"),
            ("work", "Work older"),
        ]
    );
}

#[test]
fn flat_windows_to_the_limit_newest_first_and_reports_total() {
    // Five messages, a window of two: only the two newest are built, but `total` still
    // reports all five so the host knows it can show more.
    let messages: Vec<AccountMessage> = (0..5)
        .map(|i| {
            at(
                "a",
                &message(&format!("m{i}"), &format!("msg {i}"), i * 10, None),
            )
        })
        .collect();
    let snapshot = build(&messages, &[], &[], vec![], None, None, ViewMode::Flat, 2);
    assert_eq!(
        snapshot.total, 5,
        "total counts every message, not just the window"
    );
    assert_eq!(
        flat_subjects(&snapshot),
        vec!["msg 4", "msg 3"],
        "the window is the newest `limit`, in order"
    );
}

#[test]
fn flat_window_at_or_above_total_returns_everything() {
    let messages = vec![
        at("a", &message("m1", "Older", 0, None)),
        at("a", &message("m2", "Newer", 30, None)),
    ];
    let snapshot = build(&messages, &[], &[], vec![], None, None, ViewMode::Flat, 100);
    assert_eq!(snapshot.total, 2);
    assert_eq!(
        snapshot.rows.len(),
        2,
        "a limit past the end returns every row"
    );
}

#[test]
fn flat_row_date_drops_sub_second_precision_for_a_parseable_instant() {
    let mut msg = message("m1", "Fractional", 30, None);
    msg.received_at = Some("2026-06-01T09:30:45.123Z".parse().unwrap());
    let row = flat_row(&at("a", &msg));
    assert_eq!(row.date, "2026-06-01T09:30:45Z");
}

#[test]
fn flat_row_reflects_the_flagged_keyword() {
    use engine_core::mail::{Keyword, SystemKeyword};

    let mut flagged = message("m1", "Important", 0, None);
    flagged
        .keywords
        .insert(Keyword::system(SystemKeyword::Flagged));
    let plain = message("m2", "Ordinary", 5, None);
    let snapshot = build(
        &[at("a", &flagged), at("a", &plain)],
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Flat,
        ALL,
    );

    let flags: Vec<bool> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.flagged,
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    // Newest first: m2 (:05, unflagged) then m1 (:00, flagged).
    assert_eq!(flags, vec![false, true]);
}

#[test]
fn flat_and_search_rows_reflect_the_provider_attachment_flag() {
    let mut attached = message("m1", "With file", 0, None);
    attached.has_attachment = true;
    let plain = message("m2", "Plain", 5, None);
    let snapshot = build(
        &[at("a", &attached.clone()), at("a", &plain.clone())],
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Flat,
        ALL,
    );
    let flags: Vec<bool> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.has_attachment,
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    assert_eq!(flags, vec![false, true]);

    // A search row carries the same flag, and, like the list, is ordered newest first, so
    // the attached (older) message is the second row.
    let search = search_results(&[at("a", &attached), at("a", &plain)], &[], vec![], ALL);
    let search_flags: Vec<bool> = search
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.has_attachment,
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    assert_eq!(search_flags, vec![false, true]);
}

#[test]
fn flat_row_from_is_the_sender_name_and_falls_back_to_the_email() {
    // A list row shows who a message is *from* by name, only when the header carried no name
    // does the address stand in. (The reading view keeps the full `Name <email>`.)
    let named = with_from(
        message("m1", "Named", 10, None),
        Some("Tom de Vries"),
        "tom@x.example",
    );
    let bare = with_from(message("m2", "Bare", 5, None), None, "nobody@x.example");
    let anon = message("m3", "Anon", 0, None); // no From header at all
    let snapshot = build(
        &[at("a", &named), at("a", &bare), at("a", &anon)],
        &[],
        &[],
        vec![],
        None,
        None,
        ViewMode::Flat,
        ALL,
    );
    let froms: Vec<&str> = snapshot
        .rows
        .iter()
        .map(|row| match row {
            SnapshotRow::Flat(r) => r.from.as_str(),
            SnapshotRow::Thread(_) => unreachable!(),
        })
        .collect();
    // Newest first: named (:10), bare (:05), anon (:00).
    assert_eq!(froms, vec!["Tom de Vries", "nobody@x.example", ""]);
}

#[test]
fn flat_shows_only_in_scope_messages_and_labels_the_selected_folder() {
    // The app marks which messages belong to the shown folder via `in_scope`; the flat list
    // shows only those. `selected_folder` no longer filters; it only labels the sidebar row.
    let inbox_msg = at("a", &message("m1", "Inbox msg", 0, None)); // in scope
    let sent_msg = member("a", &message("m2", "Sent msg", 5, None), false); // out of scope
    let messages = vec![inbox_msg, sent_msg];
    let folders = vec![
        Mailbox::new(MailboxId::try_from("inbox").unwrap(), "Inbox"),
        Mailbox::new(MailboxId::try_from("sent").unwrap(), "Sent"),
    ];

    let snapshot = build(
        &messages,
        &folders,
        &[],
        vec![],
        Some("a"),
        Some("inbox"),
        ViewMode::Flat,
        ALL,
    );
    assert_eq!(flat_subjects(&snapshot), vec!["Inbox msg"]);
    assert_eq!(
        snapshot.folders.len(),
        2,
        "the sidebar still lists every folder"
    );
    assert_eq!(snapshot.selected_account.as_deref(), Some("a"));
    assert_eq!(snapshot.selected.as_deref(), Some("inbox"));
}

#[test]
fn flat_order_is_independent_of_the_supplied_order_when_dates_tie() {
    // Regression: ordering only by `received_at` left same-instant messages in whatever order the
    // caller collected them, and the app's live message cache and its store reload need not agree.
    // Hosts reconcile rows by id, so the differing order became a list-wide move storm; on WinUI
    // that destroys and re-creates every moved row's container, blanking the list.
    let one = at("a", &message("m1", "One", 30, None));
    let two = at("a", &message("m2", "Two", 30, None));
    let three = at("b", &message("m3", "Three", 30, None));
    let keys = |messages: Vec<AccountMessage>| {
        build(&messages, &[], &[], vec![], None, None, ViewMode::Flat, ALL)
            .rows
            .iter()
            .map(|row| match row {
                SnapshotRow::Flat(f) => (f.account.clone(), f.key.clone()),
                SnapshotRow::Thread(_) => panic!("flat mode projects flat rows"),
            })
            .collect::<Vec<_>>()
    };

    let forward = keys(vec![one.clone(), two.clone(), three.clone()]);
    let reversed = keys(vec![three, two, one]);
    assert_eq!(
        forward, reversed,
        "tied messages must project in the same order whatever order they arrive in",
    );
    // And that order is the `(account, key)` tiebreak, not the input's.
    let expected = [("a", "m1"), ("a", "m2"), ("b", "m3")];
    assert!(
        forward
            .iter()
            .zip(expected)
            .all(|((account, key), (a, k))| account == a && key == k),
        "got {forward:?}",
    );
}
