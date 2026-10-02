//! What a folder and the list header are called, and that they agree.
//!
//! Beside [`super::folder_names`] rather than in the pane's own tests: these two functions are
//! what three other places call to name a folder, so a failure here is a rename that reached one
//! site and not the others (`docs/folder-pane.md`, rule 13).

use mailcal_bindings::{AccountFolderRow, AccountRow, FolderRole, FolderRow, MailboxListSnapshot};

use super::{folder_label, header_title};
use crate::ui::model::empty_mailbox;

fn folder(key: &str, name: &str, role: Option<FolderRole>) -> FolderRow {
    FolderRow {
        key: key.to_owned(),
        name: name.to_owned(),
        role,
        unread: 0,
        parent: None,
        depth: 0,
        has_children: false,
        expanded: false,
        visible: true,
        pending: false,
        in_trash: false,
        editable: false,
        accepts_folders: false,
        accepts_messages: false,
    }
}

/// Two accounts whose trees both hold an `inbox`: a folder key is unique only *within* its
/// account, which is what the last assertion below turns on.
fn two_accounts() -> MailboxListSnapshot {
    MailboxListSnapshot {
        accounts: vec![
            AccountRow {
                id: "acct-1".to_owned(),
                email: "eva.jansen@example.test".to_owned(),
                name: String::new(),
                expanded: true,
            },
            AccountRow {
                id: "acct-2".to_owned(),
                email: "Research & Development".to_owned(),
                name: String::new(),
                expanded: true,
            },
        ],
        account_folders: vec![
            AccountFolderRow {
                account_id: "acct-1".to_owned(),
                manages_folders: false,
                folders: vec![
                    folder("inbox", "INBOX", Some(FolderRole::Inbox)),
                    folder("custom", "Sales & Marketing", None),
                ],
            },
            AccountFolderRow {
                account_id: "acct-2".to_owned(),
                manages_folders: false,
                folders: vec![folder("inbox", "INBOX", Some(FolderRole::Inbox))],
            },
        ],
        ..empty_mailbox()
    }
}

#[test]
fn a_role_bearing_folder_is_named_by_the_app_and_every_other_keeps_its_name() {
    assert_eq!(folder_label(Some(&FolderRole::Inbox), "INBOX"), "Inbox");
    assert_eq!(
        folder_label(Some(&FolderRole::Trash), "Deleted Items"),
        "Trash"
    );
    // Renamed on the server, and still called what we call it; the trade rule 12 names.
    assert_eq!(
        folder_label(Some(&FolderRole::Archive), "Archief 2024"),
        "Archive"
    );
    // `Other` collapses flagged, important and all-mail, so there is no one honest word for it.
    assert_eq!(
        folder_label(Some(&FolderRole::Other), "All Mail"),
        "All Mail"
    );
    assert_eq!(folder_label(None, "Sales & Marketing"), "Sales & Marketing");
}

#[test]
fn the_list_header_names_the_scope_the_same_way_the_pane_does() {
    let mut snapshot = two_accounts();
    assert_eq!(header_title(&snapshot), "Inbox");

    // An account with no folder chosen is that account's whole mailbox.
    snapshot.selected_account = Some("acct-1".to_owned());
    assert_eq!(header_title(&snapshot), "All Mail");

    // A folder is named by the app, exactly as its row is: never `INBOX`.
    snapshot.selected = Some("inbox".to_owned());
    assert_eq!(header_title(&snapshot), "Inbox");
    snapshot.selected = Some("custom".to_owned());
    assert_eq!(header_title(&snapshot), "Sales & Marketing");

    // The key resolves within the *selected* account, not across the pane: acct-2 has no
    // `custom`, so the header must not borrow acct-1's row for it.
    snapshot.selected_account = Some("acct-2".to_owned());
    assert_eq!(header_title(&snapshot), "Mail");
}
