//! The cross-account search merge: pooled hits ordered newest first, ties broken to a total
//! order, and the overall cap; with `total` equal to the returned rows (no "show more").

use super::*;

#[test]
fn search_total_equals_returned_rows_so_there_is_no_show_more() {
    let hits = vec![
        at("a", &message("m1", "one", 0, None)),
        at("a", &message("m2", "two", 1, None)),
    ];
    let snapshot = search_results(&hits, &[], vec![], 100);
    assert_eq!(snapshot.total, snapshot.rows.len());
    assert_eq!(snapshot.total, 2);
}

#[test]
fn search_results_order_one_accounts_hits_newest_first() {
    // The engine hands its hits over best-scoring first; the merge re-orders them by date,
    // because a person scanning search results reads them as a mailbox, not as a ranking.
    let hits = vec![
        at("a", &message("m1", "Top hit (older)", 0, None)),
        at("a", &message("m2", "Lower hit (newer)", 30, None)),
    ];
    let snapshot = search_results(&hits, &[], vec![], 100);
    assert_eq!(
        flat_subjects(&snapshot),
        vec!["Lower hit (newer)", "Top hit (older)"]
    );
    assert_eq!(snapshot.mode, ViewMode::Flat);
}

#[test]
fn search_results_merge_accounts_by_date_not_concatenation() {
    // Two accounts, supplied grouped (all of A, then all of B) as the app does. Their
    // messages interleave in time, so the merged list must interleave them; proving the
    // result is a chronological merge, not "all of A then all of B".
    let hits = vec![
        at("a", &message("a1", "A newest", 3, None)),
        at("a", &message("a2", "A oldest", 0, None)),
        at("b", &message("b1", "B newer", 2, None)),
        at("b", &message("b2", "B older", 1, None)),
    ];
    let snapshot = search_results(&hits, &[], vec![], 100);
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
            ("a", "A newest"),
            ("b", "B newer"),
            ("b", "B older"),
            ("a", "A oldest"),
        ]
    );
}

#[test]
fn search_results_break_date_ties_on_the_row_key() {
    // Two accounts' hits share an instant (a mailing list delivered to both). The tie breaks
    // on `(account, key)`, so the sequence is the same however the app supplied them: a host
    // reconciling rows by id never sees a phantom reshuffle.
    let forward = vec![
        at("b", &message("b1", "B copy", 5, None)),
        at("a", &message("a1", "A copy", 5, None)),
    ];
    let reversed = vec![
        at("a", &message("a1", "A copy", 5, None)),
        at("b", &message("b1", "B copy", 5, None)),
    ];
    assert_eq!(
        flat_subjects(&search_results(&forward, &[], vec![], 100)),
        vec!["A copy", "B copy"]
    );
    assert_eq!(
        flat_subjects(&search_results(&reversed, &[], vec![], 100)),
        vec!["A copy", "B copy"]
    );
}

#[test]
fn search_results_order_a_sent_copy_by_its_sent_instant() {
    // A Sent copy can carry only `sent_at` (no delivery date). It must sort, and show a date
    //, by when it was sent, rather than sinking below every received message.
    let mut sent = message("s1", "Sent reply", 0, None);
    sent.sent_at = sent
        .received_at
        .map(|_| "2026-06-01T09:45:00Z".parse().unwrap());
    sent.received_at = None;
    let hits = vec![
        at("a", &message("m1", "Received earlier", 30, None)),
        at("a", &sent),
    ];
    let snapshot = search_results(&hits, &[], vec![], 100);
    assert_eq!(
        flat_subjects(&snapshot),
        vec!["Sent reply", "Received earlier"]
    );
    let SnapshotRow::Flat(row) = &snapshot.rows[0] else {
        panic!("expected a flat row");
    };
    assert_eq!(row.date, "2026-06-01T09:45:00Z");
}

#[test]
fn search_results_cap_the_merged_list_at_the_limit() {
    // The overall cap applies after the merge: the newest hits survive, the oldest are
    // dropped, whichever account they came from.
    let hits = vec![
        at("a", &message("a1", "keep (newest)", 9, None)),
        at("b", &message("b1", "keep (newer)", 5, None)),
        at("b", &message("b2", "drop (oldest)", 1, None)),
    ];
    let snapshot = search_results(&hits, &[], vec![], 2);
    assert_eq!(
        flat_subjects(&snapshot),
        vec!["keep (newest)", "keep (newer)"]
    );
    assert_eq!(snapshot.total, 2);
}
