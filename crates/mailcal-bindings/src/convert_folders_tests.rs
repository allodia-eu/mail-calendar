//! The folder intents crossing the FFI: each change names its folder by account and key
//! together, and a malformed reference drops the whole intent.

use mailcal_app::{FolderIntent as AppFolderIntent, RowRef};

use super::folder_intent;
use crate::{FolderIntent, SelectedRow};

#[test]
fn a_rename_binds_its_account_and_key_into_one_reference() {
    let AppFolderIntent::Rename { folder, name } = folder_intent(FolderIntent::Rename {
        account: "acct-1".into(),
        key: "Work".into(),
        name: "Clients".into(),
    })
    .unwrap() else {
        panic!("a rename stays a rename");
    };
    assert_eq!(folder.account.as_str(), "acct-1");
    assert_eq!(folder.key, "Work");
    assert_eq!(name, "Clients");
}

#[test]
fn dropped_rows_cross_as_rows_of_their_own_accounts() {
    let AppFolderIntent::MoveMessages { rows, folder } =
        folder_intent(FolderIntent::MoveMessages {
            rows: vec![
                SelectedRow::Message {
                    account: "acct-1".into(),
                    key: "m1".into(),
                },
                SelectedRow::Thread {
                    account: "acct-1".into(),
                    thread_id: "t1".into(),
                },
            ],
            account: "acct-1".into(),
            key: "Work".into(),
        })
        .unwrap()
    else {
        panic!("a drop stays a drop");
    };
    assert_eq!(folder.key, "Work");
    assert!(matches!(rows[0], RowRef::Message(ref m) if m.key.as_str() == "m1"));
    assert!(matches!(rows[1], RowRef::Thread(ref t) if t.thread_id.as_str() == "t1"));
}

#[test]
fn one_malformed_row_drops_the_whole_drop() {
    let dropped = folder_intent(FolderIntent::MoveMessages {
        rows: vec![
            SelectedRow::Message {
                account: "acct-1".into(),
                key: "m1".into(),
            },
            SelectedRow::Message {
                account: "acct-1".into(),
                key: String::new(),
            },
        ],
        account: "acct-1".into(),
        key: "Work".into(),
    });
    assert!(dropped.is_err());
}

#[test]
fn a_folder_without_an_account_is_refused() {
    assert!(
        folder_intent(FolderIntent::Delete {
            account: String::new(),
            key: "Work".into(),
        })
        .is_err()
    );
    assert!(
        folder_intent(FolderIntent::Create {
            account: String::new(),
            parent: None,
            name: "Receipts".into(),
        })
        .is_err()
    );
}
