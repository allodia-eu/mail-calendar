//! Queued folder changes drawn over the stored tree, and what each row then allows.

use engine_api::{Mailbox, MailboxChange, MailboxId, MailboxPlace, MailboxRole};

use super::*;
use crate::sorted_folder_rows;

fn id(value: &str) -> MailboxId {
    MailboxId::try_from(value).unwrap()
}

fn folder(key: &str, name: &str, parent: Option<&str>, role: Option<MailboxRole>) -> Mailbox {
    let mut mailbox = Mailbox::new(id(key), name);
    mailbox.parent = parent.map(id);
    mailbox.role = role;
    mailbox
}

/// Inbox, Trash holding `Old`, Junk, and `Work` holding `2024`.
fn stored() -> Vec<Mailbox> {
    vec![
        folder("inbox", "INBOX", None, Some(MailboxRole::Inbox)),
        folder("trash", "Trash", None, Some(MailboxRole::Trash)),
        folder("old", "Old", Some("trash"), None),
        folder("junk", "Junk", None, Some(MailboxRole::Junk)),
        folder("work", "Work", None, None),
        folder("w2024", "2024", Some("work"), None),
    ]
}

fn row<'a>(rows: &'a [FolderRow], key: &str) -> &'a FolderRow {
    rows.iter().find(|row| row.key == key).expect(key)
}

fn stamped(folders: &[Mailbox], pending: &BTreeSet<String>, manages: bool) -> Vec<FolderRow> {
    let mut rows = sorted_folder_rows(folders);
    stamp_folder_actions(&mut rows, manages, pending);
    rows
}

fn queued(op: u64, change: MailboxChange) -> QueuedFolderChange {
    QueuedFolderChange { op, change }
}

#[test]
fn a_folder_made_offline_is_drawn_where_it_was_made_and_marked_pending() {
    let (folders, pending) = with_folder_changes(
        &stored(),
        &[queued(
            7,
            MailboxChange::Create {
                name: "Receipts".into(),
                parent: Some(id("work")),
            },
        )],
    );
    let rows = stamped(&folders, &pending, true);

    let made = row(&rows, &pending_folder_key(7));
    assert_eq!(made.name, "Receipts");
    assert_eq!(made.parent.as_deref(), Some("work"));
    assert!(made.pending);
    // Nothing may be built on a folder the server does not have yet.
    assert!(!made.editable && !made.accepts_folders && !made.accepts_messages);
}

#[test]
fn a_queued_rename_and_move_show_at_once_and_hold_the_whole_branch() {
    let (folders, pending) = with_folder_changes(
        &stored(),
        &[queued(
            1,
            MailboxChange::Update {
                target: id("work"),
                seen: MailboxPlace {
                    name: "Work".into(),
                    parent: None,
                },
                name: "Clients".into(),
                parent: Some(id("inbox")),
            },
        )],
    );
    let rows = stamped(&folders, &pending, true);

    let moved = row(&rows, "work");
    assert_eq!(moved.name, "Clients");
    assert_eq!(moved.parent.as_deref(), Some("inbox"));
    assert!(moved.pending);
    // On IMAP the child's key moves with its parent, so the child waits too.
    assert!(row(&rows, "w2024").pending);
    assert!(!row(&rows, "w2024").editable);
}

#[test]
fn a_trashed_folder_shows_in_trash_and_a_deleted_one_is_gone_with_its_children() {
    let (folders, pending) = with_folder_changes(
        &stored(),
        &[
            queued(
                1,
                MailboxChange::Trash {
                    target: id("w2024"),
                    seen: MailboxPlace {
                        name: "2024".into(),
                        parent: Some(id("work")),
                    },
                    trash: id("trash"),
                },
            ),
            queued(2, MailboxChange::Delete { target: id("work") }),
        ],
    );
    let rows = stamped(&folders, &pending, true);

    assert!(row(&rows, "w2024").in_trash);
    assert_eq!(row(&rows, "w2024").parent.as_deref(), Some("trash"));
    assert!(rows.iter().all(|row| row.key != "work"));
}

#[test]
fn a_change_to_a_folder_the_list_does_not_hold_draws_nothing() {
    let (folders, pending) = with_folder_changes(
        &stored(),
        &[queued(
            1,
            MailboxChange::Update {
                target: id("gone"),
                seen: MailboxPlace {
                    name: "Gone".into(),
                    parent: None,
                },
                name: "Back".into(),
                parent: None,
            },
        )],
    );
    assert_eq!(folders, stored());
    assert!(pending.is_empty());
}

#[test]
fn role_folders_keep_their_place_and_junk_takes_no_mail_by_drop() {
    let rows = stamped(&stored(), &BTreeSet::new(), true);

    let inbox = row(&rows, "inbox");
    assert!(!inbox.editable, "the app names and places a role folder");
    assert!(inbox.accepts_folders && inbox.accepts_messages);
    let trash = row(&rows, "trash");
    assert!(
        !trash.accepts_folders,
        "a folder goes to Trash by being deleted"
    );
    assert!(trash.accepts_messages);
    assert!(
        !row(&rows, "junk").accepts_messages,
        "spam is a report, not a move"
    );
    let work = row(&rows, "work");
    assert!(work.editable && work.accepts_folders && work.accepts_messages);
}

#[test]
fn a_folder_in_trash_can_be_restored_but_takes_no_new_subfolders() {
    let rows = stamped(&stored(), &BTreeSet::new(), true);

    let old = row(&rows, "old");
    assert!(old.in_trash);
    assert!(old.editable, "moving it out is how it comes back");
    assert!(!old.accepts_folders);
    assert!(!row(&rows, "work").in_trash);
}

#[test]
fn an_account_that_cannot_change_folders_offers_nothing_but_mail_drops() {
    let rows = stamped(&stored(), &BTreeSet::new(), false);

    assert!(rows.iter().all(|row| !row.editable && !row.accepts_folders));
    assert!(row(&rows, "work").accepts_messages);
}

#[test]
fn a_name_is_checked_against_its_siblings_without_case() {
    let folders = stored();
    assert_eq!(
        check_folder_name(&folders, Some("work"), "Receipts", None),
        FolderNameCheck::Valid
    );
    assert_eq!(
        check_folder_name(&folders, None, "work", None),
        FolderNameCheck::Taken
    );
    // The same name in another folder is fine.
    assert_eq!(
        check_folder_name(&folders, Some("work"), "Work", None),
        FolderNameCheck::Valid
    );
    // A folder may keep its own name, in a different case.
    assert_eq!(
        check_folder_name(&folders, None, "WORK", Some("work")),
        FolderNameCheck::Valid
    );
    assert_eq!(
        check_folder_name(&folders, None, "", None),
        FolderNameCheck::Empty
    );
    assert_eq!(
        check_folder_name(&folders, None, "Work ", None),
        FolderNameCheck::Surrounded
    );
    assert_eq!(
        check_folder_name(&folders, None, "a/b", None),
        FolderNameCheck::Separator
    );
    assert_eq!(
        check_folder_name(&folders, None, "a\u{7}b", None),
        FolderNameCheck::Control
    );
}
