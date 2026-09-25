//! What a pane row offers, where a drag may land, and what the dialogs say
//! (`docs/folder-pane.md`, rules 22 to 28).

use mailcal_bindings::{FolderNameCheck, FolderRole, FolderRow, SelectedRow};

use super::{
    Dragged, DropSpot, FolderMenuItem, accepts_drop, account_menu, ancestors, delete_copy,
    folder_menu, move_candidates, name_problem, subtree,
};
use crate::l10n;

fn row(key: &str, name: &str, parent: Option<&str>) -> FolderRow {
    FolderRow {
        key: key.to_owned(),
        name: name.to_owned(),
        role: None,
        unread: 0,
        parent: parent.map(str::to_owned),
        depth: 0,
        has_children: false,
        expanded: false,
        visible: true,
        pending: false,
        in_trash: false,
        editable: true,
        accepts_folders: true,
        accepts_messages: true,
    }
}

/// Inbox (a role folder), `Work` holding `2024` holding `Q1`, and `Home`.
fn tree() -> Vec<FolderRow> {
    let mut inbox = row("inbox", "INBOX", None);
    inbox.role = Some(FolderRole::Inbox);
    inbox.editable = false;
    vec![
        inbox,
        row("work", "Work", None),
        row("2024", "2024", Some("work")),
        row("q1", "Q1", Some("2024")),
        row("home", "Home", None),
    ]
}

fn folder_drag(key: &str, parent: Option<&str>) -> Dragged {
    Dragged::Folder {
        key: key.to_owned(),
        parent: parent.map(str::to_owned),
    }
}

fn mail() -> Dragged {
    Dragged::Mail(SelectedRow::Message {
        account: "a".to_owned(),
        key: "m1".to_owned(),
    })
}

fn spot(folders: &[FolderRow], key: &str) -> DropSpot {
    DropSpot::of(folders.iter().find(|f| f.key == key).unwrap(), folders)
}

#[test]
fn a_row_offers_what_its_flags_allow_and_a_pending_row_nothing() {
    let folders = tree();
    assert_eq!(
        folder_menu(&folders[1]),
        vec![
            FolderMenuItem::NewFolder,
            FolderMenuItem::Rename,
            FolderMenuItem::MoveTo,
            FolderMenuItem::Delete
        ]
    );
    assert_eq!(folder_menu(&folders[0]), vec![FolderMenuItem::NewFolder]);
    let mut pending = row("new", "New", None);
    pending.pending = true;
    pending.editable = false;
    pending.accepts_folders = false;
    pending.accepts_messages = false;
    assert!(folder_menu(&pending).is_empty());
    assert_eq!(account_menu(true), vec![FolderMenuItem::NewFolder]);
    assert!(account_menu(false).is_empty());
}

#[test]
fn move_to_lists_top_level_first_and_never_the_folder_or_what_is_inside_it() {
    let folders = tree();
    let candidates = move_candidates(&folders, "work");
    let parents: Vec<Option<&str>> = candidates.iter().map(|c| c.parent.as_deref()).collect();
    assert_eq!(parents, vec![None, Some("inbox"), Some("home")]);
    assert_eq!(candidates[0].label, l10n::folder_move_top_level());
    // A nested folder is named with the folders it sits in.
    let labels: Vec<String> = move_candidates(&folders, "home")
        .into_iter()
        .map(|c| c.label)
        .collect();
    assert!(
        labels.contains(&"Work / 2024 / Q1".to_owned()),
        "{labels:?}"
    );
    assert!(labels.contains(&l10n::folder_inbox().to_owned()));
}

#[test]
fn a_subtree_and_the_chain_above_a_folder_are_read_from_the_rows() {
    let folders = tree();
    let inside = subtree(&folders, "work");
    assert!(inside.contains("work") && inside.contains("2024") && inside.contains("q1"));
    assert!(!inside.contains("home"));
    assert_eq!(
        ancestors(&folders, "q1"),
        vec!["2024".to_owned(), "work".to_owned()]
    );
}

#[test]
fn a_folder_drops_only_somewhere_it_can_go_in_its_own_account() {
    let folders = tree();
    let work = folder_drag("work", None);
    assert!(accepts_drop("a", &spot(&folders, "home"), "a", &work));
    assert!(
        !accepts_drop("a", &spot(&folders, "home"), "b", &work),
        "another account"
    );
    assert!(
        !accepts_drop("a", &spot(&folders, "work"), "a", &work),
        "onto itself"
    );
    assert!(
        !accepts_drop("a", &spot(&folders, "q1"), "a", &work),
        "inside itself"
    );
    let child = folder_drag("2024", Some("work"));
    assert!(
        !accepts_drop("a", &spot(&folders, "work"), "a", &child),
        "where it is"
    );

    let account = DropSpot::Account {
        manages_folders: true,
    };
    assert!(accepts_drop("a", &account, "a", &child), "to the top");
    assert!(
        !accepts_drop("a", &account, "a", &work),
        "already at the top"
    );
    let fixed = DropSpot::Account {
        manages_folders: false,
    };
    assert!(!accepts_drop("a", &fixed, "a", &child));
}

#[test]
fn mail_drops_on_a_folder_that_takes_it_and_never_on_an_account_row() {
    let mut folders = tree();
    assert!(accepts_drop("a", &spot(&folders, "home"), "a", &mail()));
    assert!(!accepts_drop("a", &spot(&folders, "home"), "b", &mail()));
    folders[4].accepts_messages = false;
    assert!(!accepts_drop("a", &spot(&folders, "home"), "a", &mail()));
    let account = DropSpot::Account {
        manages_folders: true,
    };
    assert!(!accepts_drop("a", &account, "a", &mail()));
}

#[test]
fn a_pending_row_takes_no_drop() {
    let mut folders = tree();
    folders[4].pending = true;
    folders[4].accepts_folders = false;
    folders[4].accepts_messages = false;
    assert!(!accepts_drop("a", &spot(&folders, "home"), "a", &mail()));
    assert!(!accepts_drop(
        "a",
        &spot(&folders, "home"),
        "a",
        &folder_drag("work", None)
    ));
}

#[test]
fn the_name_dialog_says_why_a_name_cannot_be_used_but_not_before_one_is_typed() {
    assert_eq!(name_problem(FolderNameCheck::Valid), None);
    assert_eq!(name_problem(FolderNameCheck::Empty), None);
    assert_eq!(
        name_problem(FolderNameCheck::Taken),
        Some(l10n::folder_name_taken())
    );
    assert_eq!(
        name_problem(FolderNameCheck::Separator),
        Some(l10n::folder_name_separator())
    );
    assert_eq!(
        name_problem(FolderNameCheck::Surrounded),
        Some(l10n::folder_name_surrounded())
    );
    assert_eq!(
        name_problem(FolderNameCheck::Control),
        Some(l10n::folder_name_control())
    );
}

#[test]
fn delete_is_a_move_to_trash_outside_it_and_permanent_inside_it() {
    let outside = delete_copy("Work", false);
    assert_eq!(outside.title, l10n::folder_delete_title("Work"));
    assert_eq!(outside.confirm, l10n::action_move_to_trash());
    assert!(!outside.destructive);
    let inside = delete_copy("Work", true);
    assert_eq!(inside.title, l10n::folder_delete_permanent_title("Work"));
    assert_eq!(inside.message, l10n::folder_delete_permanent_message());
    assert_eq!(inside.confirm, l10n::action_delete_permanently());
    assert!(inside.destructive);
}
