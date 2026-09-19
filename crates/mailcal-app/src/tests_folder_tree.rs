//! The folder tree in the pane, end to end through the intents the clients dispatch: which
//! rows are on screen, which carry a disclosure control, and what shutting one does.
//!
//! The layer every client shares, so the rules hold once rather than four times. Two of them
//! are ones a client cannot check for itself: that shutting a folder is **not** selecting it
//! (a chevron that navigated would make the pane rearrange itself, which rule 2 exists to
//! prevent), and that the answer is the same after a relaunch.
//!
//! The contract is `docs/folder-pane.md`.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app, app_with_prefs, message};
use mailcal_viewmodel::FolderRow;

use super::{Intent, reference::FolderRef};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The rows a pane would actually draw, as `(key, depth)`.
fn drawn(rows: &[FolderRow]) -> Vec<(&str, u32)> {
    rows.iter()
        .filter(|row| row.visible)
        .map(|row| (row.key.as_str(), row.depth))
        .collect()
}

/// A folder of the one account these tests use.
fn folder(key: &str) -> FolderRef {
    FolderRef {
        account: engine_api::AccountId::try_from("acct-1").unwrap(),
        key: key.to_owned(),
    }
}

/// One account whose Archive holds `Clients`, which holds `Acme`.
fn nested_app(surfaces: &Arc<Mutex<Vec<super::Surface>>>) -> super::App<FakeProvider> {
    app(
        vec![account(
            "acct-1",
            FakeProvider::with_archive(vec![message("m1", "a", "One")]).with_nested_folders(),
        )],
        surfaces,
    )
}

#[tokio::test]
async fn a_folder_inside_a_folder_is_drawn_under_it_and_can_be_shut() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = nested_app(&surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // Nobody has shut anything, so the whole tree is on screen and each level is indented
    // one step further than the one holding it.
    let rows = app.mailbox_list().account_folders[0].folders.clone();
    assert_eq!(
        drawn(&rows),
        vec![
            ("a", 0),
            ("archive", 0),
            ("archive/clients", 1),
            ("archive/clients/acme", 2)
        ]
    );
    // A disclosure control belongs only where there is a tree to open.
    let holds: Vec<bool> = rows.iter().map(|row| row.has_children).collect();
    assert_eq!(holds, vec![false, true, true, false]);
    assert!(
        rows[1].expanded,
        "a folder nobody has shut shows what is in it"
    );

    app.dispatch(Intent::SetFolderExpanded {
        folder: folder("archive/clients"),
        expanded: false,
    })
    .await;

    // Shutting `Clients` takes what is inside it off screen and nothing else: `Clients` itself
    // is still a row, and the Archive above it is untouched.
    let snapshot = app.mailbox_list();
    let rows = snapshot.account_folders[0].folders.clone();
    assert_eq!(
        drawn(&rows),
        vec![("a", 0), ("archive", 0), ("archive/clients", 1)]
    );
    assert!(!rows[2].expanded);
    assert!(rows[1].expanded);

    // And shutting a folder is not opening it. This is the half no client can check alone.
    assert!(snapshot.selected.is_none());
    assert!(snapshot.selected_account.is_none());
}

#[tokio::test]
async fn shutting_a_folder_hides_the_whole_branch_beneath_it() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = nested_app(&surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // Shutting the Archive hides `Clients` **and** the `Acme` inside it: a row is on screen
    // only while every folder above it is open, not merely its own parent.
    app.dispatch(Intent::SetFolderExpanded {
        folder: folder("archive"),
        expanded: false,
    })
    .await;

    assert_eq!(
        drawn(&app.mailbox_list().account_folders[0].folders),
        vec![("a", 0), ("archive", 0)]
    );

    // Opening it again brings back exactly what was showing before, including the state of
    // the folders inside it, which nothing in between touched.
    app.dispatch(Intent::SetFolderExpanded {
        folder: folder("archive"),
        expanded: true,
    })
    .await;
    assert_eq!(
        drawn(&app.mailbox_list().account_folders[0].folders),
        vec![
            ("a", 0),
            ("archive", 0),
            ("archive/clients", 1),
            ("archive/clients/acme", 2)
        ]
    );
}

#[tokio::test]
async fn a_shut_folder_is_still_shut_after_a_relaunch() {
    // The reason expansion lives in the core at all (rule 3). A client holding this in view
    // state looks perfectly correct until the app is restarted.
    let dir = std::env::temp_dir().join("mailcal-folder-tree-relaunch");
    let _ = std::fs::remove_dir_all(&dir);
    let surfaces = Arc::new(Mutex::new(Vec::new()));

    {
        let app = app_with_prefs(
            vec![account(
                "acct-1",
                FakeProvider::with_archive(vec![message("m1", "a", "One")]).with_nested_folders(),
            )],
            dir.join("preferences.toml"),
            &surfaces,
        );
        app.dispatch(Intent::RefreshMail).await;
        app.dispatch(Intent::SetFolderExpanded {
            folder: folder("archive"),
            expanded: false,
        })
        .await;
    }

    let app = app_with_prefs(
        vec![account(
            "acct-1",
            FakeProvider::with_archive(vec![message("m1", "a", "One")]).with_nested_folders(),
        )],
        dir.join("preferences.toml"),
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        drawn(&app.mailbox_list().account_folders[0].folders),
        vec![("a", 0), ("archive", 0)]
    );
}
