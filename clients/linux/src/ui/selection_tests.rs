//! The selection rules, without a display: what each modifier means, what survives a snapshot
//! rebuild, and how many rows the reading pane says are picked.

use mailcal_bindings::{FlatRow, SelectedRow, SnapshotRow, ThreadRow, ViewMode};

use super::{SelectMode, Selection};

fn flat(key: &str, unread: bool, flagged: bool) -> SnapshotRow {
    let mut avatar = crate::ui::model::blank_avatar();
    avatar.initials = "S".to_owned();
    SnapshotRow::Flat {
        row: FlatRow {
            avatar,
            account: "acct-1".to_owned(),
            key: key.to_owned(),
            subject: format!("Subject {key}"),
            from: "sender".to_owned(),
            date: "2026-07-20".to_owned(),
            unread,
            flagged,
            draft: false,
            has_attachment: false,
            preview: String::new(),
        },
    }
}

fn thread(id: &str, unread_count: u32) -> SnapshotRow {
    let mut avatar = crate::ui::model::blank_avatar();
    avatar.initials = "T".to_owned();
    SnapshotRow::Thread {
        row: ThreadRow {
            avatar,
            account: "acct-1".to_owned(),
            thread_id: id.to_owned(),
            latest_key: format!("{id}-latest"),
            subject: "Conversation".to_owned(),
            latest_from: "sender".to_owned(),
            latest_date: "2026-07-20".to_owned(),
            message_count: 3,
            unread_count,
            flagged: false,
            has_attachment: false,
            preview: String::new(),
            messages: Vec::new(),
        },
    }
}

fn keys(selection: &Selection) -> Vec<String> {
    selection
        .selected_rows()
        .into_iter()
        .map(|row| match row {
            SelectedRow::Message { key, .. } => key,
            SelectedRow::Thread { thread_id, .. } => thread_id,
        })
        .collect()
}

#[test]
fn a_plain_click_picks_one_row_and_drops_the_rest() {
    let rows = [flat("m1", false, false), flat("m2", false, false)];
    let mut selection = Selection::default();

    selection.click(&rows, 0, SelectMode::Toggle);
    selection.click(&rows, 1, SelectMode::Toggle);
    assert_eq!(keys(&selection), ["m1", "m2"]);

    selection.click(&rows, 1, SelectMode::Replace);
    assert_eq!(keys(&selection), ["m2"], "a plain click starts over");
}

#[test]
fn ctrl_click_adds_then_removes_the_same_row() {
    let rows = [flat("m1", false, false), flat("m2", false, false)];
    let mut selection = Selection::default();

    selection.click(&rows, 0, SelectMode::Replace);
    selection.click(&rows, 1, SelectMode::Toggle);
    assert_eq!(keys(&selection), ["m1", "m2"]);

    selection.click(&rows, 1, SelectMode::Toggle);
    assert_eq!(keys(&selection), ["m1"], "the second click deselects");
}

#[test]
fn shift_click_takes_the_range_between_the_anchor_and_the_row() {
    let rows = [
        flat("m1", false, false),
        flat("m2", false, false),
        flat("m3", false, false),
        flat("m4", false, false),
    ];
    let mut selection = Selection::default();

    selection.click(&rows, 1, SelectMode::Replace);
    selection.click(&rows, 3, SelectMode::Range);
    assert_eq!(keys(&selection), ["m2", "m3", "m4"]);

    // Upwards from the same anchor, which is what a user correcting an over-long range does.
    selection.click(&rows, 0, SelectMode::Range);
    assert_eq!(keys(&selection), ["m1", "m2"]);
}

#[test]
fn a_shift_click_with_nothing_to_extend_from_picks_the_one_row() {
    let rows = [flat("m1", false, false), flat("m2", false, false)];
    let mut selection = Selection::default();

    selection.click(&rows, 1, SelectMode::Range);
    assert_eq!(keys(&selection), ["m2"]);
}

#[test]
fn a_row_that_leaves_the_list_leaves_the_selection_with_it() {
    // The archived rows are gone from the next snapshot. A selection that kept them would act on
    // messages nobody can see, and the first sign of it would be mail leaving the mailbox.
    let rows = [
        flat("m1", false, false),
        flat("m2", false, false),
        flat("m3", false, false),
    ];
    let mut selection = Selection::default();
    selection.select_all(&rows);

    selection.retain_listed(&[flat("m2", false, false)]);
    assert_eq!(keys(&selection), ["m2"]);

    selection.retain_listed(&[]);
    assert!(
        selection.is_empty(),
        "an emptied list empties the selection"
    );
}

/// A move names its rows as the menu or the bar saw them, in whatever order, and they leave the
/// selection as the move starts rather than when the list stops showing them
/// (`docs/list-selection.md`, rule 12).
#[test]
fn rows_a_move_takes_leave_the_selection_whatever_order_it_named_them_in() {
    let rows = [
        flat("m1", false, false),
        flat("m2", false, false),
        flat("m3", false, false),
    ];
    let mut selection = Selection::default();
    selection.select_all(&rows);
    let named = |key: &str| SelectedRow::Message {
        account: "acct-1".to_owned(),
        key: key.to_owned(),
    };

    selection.forget(&[named("m3"), named("m1")]);
    assert_eq!(keys(&selection), ["m2"]);
    // The anchor went with m1, so a Shift-click has nothing to extend from.
    selection.click(&rows, 2, SelectMode::Range);
    assert_eq!(keys(&selection), ["m3"]);

    selection.forget(&[named("m3")]);
    assert!(selection.is_empty());
}

#[test]
fn a_range_after_its_anchor_left_the_list_picks_the_one_row() {
    let rows = [flat("m1", false, false), flat("m2", false, false)];
    let mut selection = Selection::default();
    selection.click(&rows, 0, SelectMode::Replace);

    // m1 was archived; the anchor goes with it rather than pointing at whatever moved up.
    let rows = [flat("m2", false, false), flat("m3", false, false)];
    selection.retain_listed(&rows);
    selection.click(&rows, 1, SelectMode::Range);
    assert_eq!(keys(&selection), ["m3"]);
}

/// A plain click selects a row *and* opens it, so a pane stating "1 selected" would replace the
/// message the user just asked to read with a count of it.
#[test]
fn the_reading_pane_states_the_count_only_above_one_row() {
    let rows = [flat("m1", false, false), flat("m2", false, false)];
    let mut selection = Selection::default();
    assert_eq!(
        selection.summary().pane_label(&ViewMode::Flat),
        None,
        "nothing picked, nothing to say"
    );

    selection.click(&rows, 0, SelectMode::Replace);
    assert_eq!(
        selection.summary().pane_label(&ViewMode::Flat),
        None,
        "the one row is what the pane is showing"
    );

    selection.select_all(&rows);
    let summary = selection.summary();
    // The words are the catalog's; what this asserts is that the list the rows were picked in
    // decides which noun, since a threaded row stands for a whole conversation.
    assert!(summary.pane_label(&ViewMode::Flat).is_some());
    assert_ne!(
        summary.pane_label(&ViewMode::Flat),
        summary.pane_label(&ViewMode::Threaded),
    );
}

#[test]
fn a_conversation_is_selected_as_a_thread_not_as_its_latest_message() {
    // The core expands a conversation itself, from the store's thread index; naming its latest
    // message here would archive one reply and leave the rest of the thread in the inbox.
    let rows = [thread("t1", 2)];
    let mut selection = Selection::default();
    selection.click(&rows, 0, SelectMode::Replace);

    assert_eq!(
        selection.selected_rows(),
        vec![SelectedRow::Thread {
            account: "acct-1".to_owned(),
            thread_id: "t1".to_owned(),
        }],
    );
}
