//! What a message row's Move to folder… offers (`docs/folder-pane.md`, rule 24), and which rows
//! it acts on (`docs/list-selection.md`, rule 12).

use mailcal_bindings::{AccountFolderRow, FolderRole, FolderRow, SelectedRow};

use super::{MoveContext, plan};

fn folder(key: &str, role: Option<FolderRole>, parent: Option<&str>) -> FolderRow {
    FolderRow {
        key: key.to_owned(),
        name: key.to_owned(),
        role,
        unread: 0,
        parent: parent.map(str::to_owned),
        depth: 0,
        has_children: false,
        expanded: false,
        visible: true,
        pending: false,
        in_trash: false,
        editable: false,
        accepts_folders: false,
        accepts_messages: true,
    }
}

/// Account `a`: Inbox, Junk and Drafts (which take no mail), `Work` holding `Q1`, and `New`,
/// still being made. Account `b`: its Inbox and `Home`.
fn context(selected: Vec<SelectedRow>, showing: Option<&str>) -> MoveContext {
    let mut junk = folder("junk", Some(FolderRole::Junk), None);
    junk.accepts_messages = false;
    let mut drafts = folder("drafts", Some(FolderRole::Drafts), None);
    drafts.accepts_messages = false;
    let mut made = folder("New", None, None);
    made.pending = true;
    made.accepts_messages = false;
    let mut snapshot = crate::ui::model::empty_mailbox();
    snapshot.account_folders = vec![
        AccountFolderRow {
            account_id: "a".to_owned(),
            folders: vec![
                folder("inbox", Some(FolderRole::Inbox), None),
                junk,
                drafts,
                folder("Work", None, None),
                folder("Q1", None, Some("Work")),
                made,
            ],
            manages_folders: true,
        },
        AccountFolderRow {
            account_id: "b".to_owned(),
            folders: vec![
                folder("inbox-b", Some(FolderRole::Inbox), None),
                folder("Home", None, None),
            ],
            manages_folders: true,
        },
    ];
    snapshot.selected_account = showing.map(|_| "a".to_owned());
    snapshot.selected = showing.map(str::to_owned);
    MoveContext::new(&snapshot, selected, false)
}

fn message(account: &str, key: &str) -> SelectedRow {
    SelectedRow::Message {
        account: account.to_owned(),
        key: key.to_owned(),
    }
}

fn keys(context: &MoveContext, row: &SelectedRow) -> Option<Vec<String>> {
    plan(context, row).map(|plan| plan.choices.into_iter().map(|c| c.key).collect())
}

#[test]
fn lists_the_folders_that_take_mail_leaving_out_the_one_on_screen() {
    let context = context(Vec::new(), Some("inbox"));
    assert_eq!(
        keys(&context, &message("a", "m1")),
        Some(vec!["Work".to_owned(), "Q1".to_owned()]),
        "not the Inbox on screen, not Junk or Drafts, not a folder still being made",
    );
}

#[test]
fn names_each_folder_by_its_path() {
    let context = context(Vec::new(), None);
    let plan = plan(&context, &message("a", "m1")).unwrap();
    assert_eq!(plan.account, "a");
    let q1 = plan.choices.iter().find(|c| c.key == "Q1").unwrap();
    assert_eq!(q1.label, "Work / Q1");
}

#[test]
fn a_row_in_the_selection_acts_on_the_selection() {
    let selection = vec![message("a", "m1"), message("a", "m2")];
    let context = context(selection, None);
    assert!(plan(&context, &message("a", "m2")).is_some());
}

#[test]
fn a_selection_spanning_accounts_offers_nothing() {
    let selection = vec![message("a", "m1"), message("b", "m9")];
    let context = context(selection, None);
    assert_eq!(plan(&context, &message("a", "m1")), None);
    // A row outside that selection is about itself alone.
    assert_eq!(
        keys(&context, &message("b", "m7")),
        Some(vec!["inbox-b".to_owned(), "Home".to_owned()])
    );
}

#[test]
fn a_conversation_row_is_offered_the_same_folders() {
    let thread = SelectedRow::Thread {
        account: "a".to_owned(),
        thread_id: "t1".to_owned(),
    };
    let context = context(vec![thread.clone()], Some("Work"));
    assert_eq!(
        keys(&context, &thread),
        Some(vec!["inbox".to_owned(), "Q1".to_owned()])
    );
}

#[test]
fn nothing_is_offered_when_no_folder_is_left() {
    let mut snapshot = crate::ui::model::empty_mailbox();
    snapshot.account_folders = vec![AccountFolderRow {
        account_id: "a".to_owned(),
        folders: vec![folder("inbox", Some(FolderRole::Inbox), None)],
        manages_folders: false,
    }];
    snapshot.selected_account = Some("a".to_owned());
    snapshot.selected = Some("inbox".to_owned());
    let context = MoveContext::new(&snapshot, Vec::new(), false);
    assert_eq!(plan(&context, &message("a", "m1")), None);
    // A search is not the folder it started in, so that folder is listed.
    let searching = MoveContext::new(&snapshot, Vec::new(), true);
    assert!(plan(&searching, &message("a", "m1")).is_some());
}
