//! Changing the folder tree end to end through the intents a pane dispatches: a folder made,
//! renamed, moved and deleted; one made offline drawn at once and sent on reconnect; a queued
//! change refused rather than applied over a change made elsewhere; mail dropped on a folder.
//!
//! The contract is `docs/folder-pane.md`, "Changing the tree".

use std::sync::{Arc, Mutex, atomic::Ordering};

use engine_api::{MailEdit, Mailbox, MailboxId};
use fakes::{FakeProvider, account, app, message, msg, open_folder};
use mailcal_viewmodel::{FolderAction, FolderProblem, FolderRow};

use super::{
    FolderIntent, Intent,
    reference::{FolderRef, RowRef},
};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

const ACCOUNT: &str = "acct-1";

fn folder(key: &str) -> FolderRef {
    FolderRef::from_parts(ACCOUNT, key.to_owned()).unwrap()
}

fn account_id() -> engine_api::AccountId {
    engine_api::AccountId::try_from(ACCOUNT).unwrap()
}

/// Inbox `a`, Archive, Trash, and a custom `projects`, with one message in the Inbox.
fn provider() -> FakeProvider {
    let mut provider = FakeProvider::with_trash(vec![message("m1", "a", "Hello")]);
    provider = provider.with_folder_tree();
    provider.folder_tree().lock().unwrap().push(Mailbox::new(
        MailboxId::try_from("projects").unwrap(),
        "Projects",
    ));
    provider
}

fn tree_app(provider: FakeProvider) -> super::App<FakeProvider> {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    app(vec![account(ACCOUNT, provider)], &surfaces)
}

fn rows(app: &super::App<FakeProvider>) -> Vec<FolderRow> {
    app.mailbox_list().account_folders[0].folders.clone()
}

fn row(rows: &[FolderRow], key: &str) -> FolderRow {
    rows.iter().find(|row| row.key == key).expect(key).clone()
}

fn server(tree: &Arc<Mutex<Vec<Mailbox>>>, key: &str) -> Option<Mailbox> {
    tree.lock()
        .unwrap()
        .iter()
        .find(|m| m.id.as_str() == key)
        .cloned()
}

async fn go_offline(app: &super::App<FakeProvider>, provider_fail: &std::sync::atomic::AtomicBool) {
    provider_fail.store(true, Ordering::SeqCst);
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
}

#[tokio::test]
async fn a_folder_is_made_inside_another_and_drawn_where_the_server_put_it() {
    let provider = provider();
    let tree = provider.folder_tree();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;
    assert!(app.mailbox_list().account_folders[0].manages_folders);

    app.dispatch(Intent::Folders(FolderIntent::Create {
        account: account_id(),
        parent: Some("projects".into()),
        name: "Receipts".into(),
    }))
    .await;

    let made = server(&tree, "made-Receipts").expect("on the server");
    assert_eq!(
        made.parent.as_ref().map(MailboxId::as_str),
        Some("projects")
    );
    let drawn = row(&rows(&app), "made-Receipts");
    assert_eq!(drawn.parent.as_deref(), Some("projects"));
    assert!(!drawn.pending, "the server has it, so it is not pending");
    assert!(app.mailbox_list().folder_notice.is_none());
}

#[tokio::test]
async fn a_folder_made_offline_is_drawn_at_once_and_made_on_reconnect() {
    let provider = provider();
    let tree = provider.folder_tree();
    let fail = provider.failure_switch();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;

    go_offline(&app, &fail).await;
    app.dispatch(Intent::Folders(FolderIntent::Create {
        account: account_id(),
        parent: None,
        name: "Travel".into(),
    }))
    .await;

    let pending: Vec<FolderRow> = rows(&app).into_iter().filter(|r| r.pending).collect();
    assert_eq!(pending.len(), 1, "drawn before the server has it");
    assert_eq!(pending[0].name, "Travel");
    assert!(pending[0].key.starts_with("pending-folder:"));
    assert!(
        !pending[0].accepts_messages,
        "no mail into a folder with no key yet"
    );
    assert!(server(&tree, "made-Travel").is_none());
    assert!(
        app.mailbox_list().folder_notice.is_none(),
        "queued is not refused"
    );

    fail.store(false, Ordering::SeqCst);
    app.dispatch(Intent::ReportNetworkReachable(true)).await;

    assert!(server(&tree, "made-Travel").is_some());
    let rows = rows(&app);
    assert!(rows.iter().all(|r| !r.pending));
    assert_eq!(row(&rows, "made-Travel").name, "Travel");
}

#[tokio::test]
async fn a_queued_rename_is_refused_when_the_folder_was_renamed_elsewhere() {
    let provider = provider();
    let tree = provider.folder_tree();
    let fail = provider.failure_switch();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;

    go_offline(&app, &fail).await;
    app.dispatch(Intent::Folders(FolderIntent::Rename {
        folder: folder("projects"),
        name: "Clients".into(),
    }))
    .await;
    assert_eq!(row(&rows(&app), "projects").name, "Clients");
    assert!(row(&rows(&app), "projects").pending);

    // Another client renames the same folder while this device is offline.
    tree.lock()
        .unwrap()
        .iter_mut()
        .find(|m| m.id.as_str() == "projects")
        .unwrap()
        .name = "Work".into();
    fail.store(false, Ordering::SeqCst);
    app.dispatch(Intent::ReportNetworkReachable(true)).await;

    assert_eq!(
        server(&tree, "projects").unwrap().name,
        "Work",
        "not overwritten"
    );
    assert_eq!(row(&rows(&app), "projects").name, "Work");
    let notice = app
        .mailbox_list()
        .folder_notice
        .expect("the refusal is said");
    assert_eq!(notice.action, FolderAction::Rename);
    assert_eq!(notice.problem, FolderProblem::ChangedElsewhere);
    assert_eq!(notice.folder, "Projects", "named as the user last saw it");

    app.dispatch(Intent::Folders(FolderIntent::DismissNotice))
        .await;
    assert!(app.mailbox_list().folder_notice.is_none());
}

#[tokio::test]
async fn a_folder_dropped_on_another_moves_inside_it_and_back_to_the_top() {
    let provider = provider();
    let tree = provider.folder_tree();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Folders(FolderIntent::Move {
        folder: folder("projects"),
        parent: Some("a".into()),
    }))
    .await;
    assert_eq!(row(&rows(&app), "projects").parent.as_deref(), Some("a"));

    app.dispatch(Intent::Folders(FolderIntent::Move {
        folder: folder("projects"),
        parent: None,
    }))
    .await;
    assert_eq!(server(&tree, "projects").unwrap().parent, None);
    assert_eq!(row(&rows(&app), "projects").parent, None);
}

#[tokio::test]
async fn deleting_puts_a_folder_in_trash_and_deleting_it_there_removes_it() {
    let provider = provider();
    let tree = provider.folder_tree();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Folders(FolderIntent::Delete(folder("projects"))))
        .await;
    let trashed = row(&rows(&app), "projects");
    assert_eq!(trashed.parent.as_deref(), Some("trash"));
    assert!(trashed.in_trash, "the next delete is the permanent one");

    app.dispatch(Intent::Folders(FolderIntent::Delete(folder("projects"))))
        .await;
    assert!(server(&tree, "projects").is_none());
    assert!(rows(&app).iter().all(|r| r.key != "projects"));
}

#[tokio::test]
async fn the_open_folder_gives_way_to_its_account_when_it_is_deleted() {
    let provider = provider();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(open_folder(ACCOUNT, "projects")).await;

    app.dispatch(Intent::Folders(FolderIntent::Delete(folder("projects"))))
        .await;
    app.dispatch(Intent::Folders(FolderIntent::Delete(folder("projects"))))
        .await;

    let list = app.mailbox_list();
    assert_eq!(list.selected_account.as_deref(), Some(ACCOUNT));
    assert_eq!(
        list.selected, None,
        "no folder that no longer exists stays open"
    );
}

#[tokio::test]
async fn mail_dropped_on_a_folder_moves_there_and_mail_of_another_account_stays() {
    let provider = provider();
    let edits = provider.edits();
    let app = tree_app(provider);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Folders(FolderIntent::MoveMessages {
        rows: vec![
            RowRef::Message(msg(ACCOUNT, "m1")),
            RowRef::Message(msg("acct-other", "m9")),
        ],
        folder: folder("projects"),
    }))
    .await;

    let edits = edits.lock().unwrap().clone();
    assert_eq!(
        edits,
        vec![MailEdit::move_to(
            engine_api::ProviderKey::new("m1").unwrap(),
            MailboxId::try_from("projects").unwrap()
        )]
    );
}

#[tokio::test]
async fn an_account_that_cannot_change_folders_offers_and_does_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let plain = FakeProvider::with_trash(vec![message("m1", "a", "Hello")]);
    let app = app(vec![account(ACCOUNT, plain)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    assert!(!app.mailbox_list().account_folders[0].manages_folders);
    assert!(rows(&app).iter().all(|r| !r.editable && !r.accepts_folders));
    app.dispatch(Intent::Folders(FolderIntent::Rename {
        folder: folder("archive"),
        name: "Old".into(),
    }))
    .await;
    assert!(app.mailbox_list().folder_notice.is_none());
}
