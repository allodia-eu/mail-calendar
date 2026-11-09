//! The command bar, and the one menu decision it was built beside: what Delete does in Trash.

use super::*;
use crate::{
    row_menu::{
        RowMenuItem,
        tests::{mail, threaded_in},
    },
    row_reach::MessageKind,
};

fn thread(id: &str) -> MenuRow {
    MenuRow::Thread {
        account: "a".to_owned(),
        thread_id: id.to_owned(),
    }
}

fn message(key: &str) -> MenuRow {
    MenuRow::Message {
        account: "a".to_owned(),
        key: key.to_owned(),
    }
}

fn commands(bar: &CommandBar) -> Vec<Vec<Command>> {
    bar.sections
        .iter()
        .map(|section| section.iter().map(|item| item.command).collect())
        .collect()
}

fn on_bar(bar: &CommandBar, action: RowAction) -> &CommandItem {
    bar.sections
        .iter()
        .flatten()
        .find(|item| item.command == Command::Act(action))
        .unwrap_or_else(|| panic!("{action:?} is not on the bar"))
}

fn menu_items(snapshot: &MailboxListSnapshot, rows: &[MenuRow]) -> Vec<RowMenuItem> {
    row_menu(snapshot, rows, true)
        .sections
        .into_iter()
        .flatten()
        .collect()
}

fn inbox() -> MailboxListSnapshot {
    threaded_in(
        "inbox",
        &[
            mail("r1", "inbox", 0, "t", MessageKind::Received),
            mail("s1", "sent", 10, "t", MessageKind::Sent),
            mail("x1", "inbox", 4, "x", MessageKind::Received),
            mail("x2", "inbox", 5, "x", MessageKind::Received),
        ],
    )
}

#[test]
fn the_bar_holds_filing_first_then_state_then_sync_and_reports_from_its_menu() {
    use RowAction::{Archive, Delete, Flag, MarkUnread, MoveToFolder};
    let bar = command_bar(&inbox(), &[thread("t")], true);
    assert_eq!(
        commands(&bar),
        [
            vec![
                Command::Act(Delete),
                Command::Act(Archive),
                Command::Act(MoveToFolder),
                Command::Act(Flag),
                Command::Act(MarkUnread),
            ],
            vec![Command::Sync],
        ]
    );
    assert_eq!(bar.menu.len(), 1);
    assert_eq!(bar.menu[0].command, Command::Act(RowAction::MarkAsSpam));
}

#[test]
fn with_nothing_selected_every_action_is_disabled_and_names_its_affirmative_half() {
    let bar = command_bar(&inbox(), &[], true);
    for item in bar.sections.iter().flatten().chain(&bar.menu) {
        let expected = item.command == Command::Sync;
        assert_eq!(item.enabled, expected, "{:?}", item.command);
    }
    on_bar(&bar, RowAction::Flag);
    on_bar(&bar, RowAction::MarkRead);
    on_bar(&bar, RowAction::Delete);
}

#[test]
fn every_bar_action_is_the_row_menus_own_item() {
    let snapshot = inbox();
    for rows in [
        vec![thread("t")],
        vec![message("x1")],
        vec![thread("t"), thread("x")],
    ] {
        let bar = command_bar(&snapshot, &rows, true);
        let menu = menu_items(&snapshot, &rows);
        for item in bar.sections.iter().flatten().chain(&bar.menu) {
            let Command::Act(action) = item.command else {
                continue;
            };
            match menu.iter().find(|m| m.action == action) {
                Some(m) => {
                    assert_eq!(item.enabled, m.enabled, "{action:?} over {rows:?}");
                    assert_eq!(item.target, m.target, "{action:?} over {rows:?}");
                }
                None => assert!(!item.enabled, "{action:?} is not in the menu over {rows:?}"),
            }
        }
    }
}

#[test]
fn a_report_over_several_rows_is_shown_and_disabled() {
    let bar = command_bar(&inbox(), &[thread("t"), thread("x")], true);
    assert_eq!(bar.menu[0].command, Command::Act(RowAction::MarkAsSpam));
    assert!(!bar.menu[0].enabled);
}

#[test]
fn a_selection_mode_has_no_sync_and_no_menu() {
    let bar = command_bar(&inbox(), &[thread("t")], false);
    assert_eq!(bar.sections.len(), 1);
    assert_eq!(bar.menu, []);
    assert!(
        bar.sections[0]
            .iter()
            .all(|item| item.command != Command::Sync)
    );
}

#[test]
fn nothing_offers_a_permanent_delete_beside_delete() {
    for snapshot in [inbox(), trash()] {
        for rows in [vec![thread("t")], vec![thread("t"), thread("x")]] {
            let menu = menu_items(&snapshot, &rows);
            let deletes = menu
                .iter()
                .filter(|m| matches!(m.action, RowAction::Delete | RowAction::PermanentlyDelete))
                .count();
            assert_eq!(deletes, 1, "{rows:?}");
        }
    }
}

fn trash() -> MailboxListSnapshot {
    threaded_in(
        "trash",
        &[
            mail("r1", "trash", 0, "t", MessageKind::Received),
            mail("s1", "sent", 10, "t", MessageKind::Sent),
            mail("x1", "trash", 4, "x", MessageKind::Received),
            mail("x2", "trash", 5, "x", MessageKind::Received),
        ],
    )
}

#[test]
fn delete_in_trash_deletes_for_good() {
    let snapshot = trash();
    let bar = command_bar(&snapshot, &[thread("t")], true);
    let delete = on_bar(&bar, RowAction::PermanentlyDelete);
    assert!(
        delete.enabled,
        "r1 is in Trash; the Sent copy stays where it is"
    );
    let both = command_bar(&snapshot, &[thread("t"), thread("x")], true);
    assert!(on_bar(&both, RowAction::PermanentlyDelete).enabled);
}

#[test]
fn delete_moves_to_trash_while_anything_it_takes_is_outside_trash() {
    // One message of the conversation is still in the Inbox: Delete throws it away first.
    let snapshot = threaded_in(
        "trash",
        &[
            mail("r1", "trash", 0, "t", MessageKind::Received),
            mail("r2", "inbox", 10, "t", MessageKind::Received),
            mail("x1", "trash", 4, "x", MessageKind::Received),
            mail("x2", "trash", 5, "x", MessageKind::Received),
        ],
    );
    let bar = command_bar(&snapshot, &[thread("t")], true);
    assert!(on_bar(&bar, RowAction::Delete).enabled);
    let mixed = command_bar(&snapshot, &[thread("t"), thread("x")], true);
    assert!(on_bar(&mixed, RowAction::Delete).enabled);
}

#[test]
fn an_account_with_no_trash_deletes_for_good() {
    let mut snapshot = inbox();
    snapshot.places[0].trash = None;
    let bar = command_bar(&snapshot, &[thread("t")], true);
    assert!(on_bar(&bar, RowAction::PermanentlyDelete).enabled);
}

#[test]
fn an_account_whose_folders_are_unknown_deletes_nothing() {
    let mut snapshot = inbox();
    snapshot.places.clear();
    let bar = command_bar(&snapshot, &[thread("t")], true);
    assert!(!on_bar(&bar, RowAction::Delete).enabled);

    // Nothing found yet, not even an Inbox: no Trash is not evidence of an account without one.
    let mut unsynced = inbox();
    unsynced.places[0].inbox = None;
    unsynced.places[0].trash = None;
    let bar = command_bar(&unsynced, &[thread("t")], true);
    assert!(!on_bar(&bar, RowAction::Delete).enabled);
}
