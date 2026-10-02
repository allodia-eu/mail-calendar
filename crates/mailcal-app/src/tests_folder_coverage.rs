//! Which folders an account pass syncs: every folder the account lists, whether or not a
//! provider was bound to it when the account connected, and never one twice because it was
//! opened.
//!
//! The account-level counterpart of the dial binding every folder
//! (`mailcal_account::connect_mail_providers`): that covers the folders listed at connect time,
//! and these cover a folder listed later and what opening a folder does once a pass has run.

use std::sync::{Arc, Mutex};

use fakes::{
    FakeConnector, FakeProvider, FlakyConnector, account, account_with, app_with_connector,
    flat_subjects, message, open_folder,
};
use mailcal_viewmodel::EmptyReason;

use super::Intent;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn a_pass_syncs_a_listed_folder_nobody_has_opened() {
    // The folder is in the account's list and bound to no provider, as one created on the
    // server after the account connected is. The pass that lists it syncs it, so its mail is
    // in the account's view before anyone opens the folder.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = FakeConnector::new(vec![(
        "projects".to_owned(),
        vec![message("p1", "projects", "Kick-off notes")],
    )]);
    let folders = connector.folders();
    let app = app_with_connector(
        vec![account(
            "acct-1",
            FakeProvider::imap_inbox(vec![message("m1", "a", "Hello")], &["projects"]),
        )],
        connector,
        &surfaces,
    );

    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Kick-off notes"),
        "the pass must sync every listed folder: {:?}",
        flat_subjects(&app.mailbox_list()),
    );

    // And every pass after it, not only the first.
    for (_, messages) in folders.lock().unwrap().iter_mut() {
        messages.push(message("p2", "projects", "Budget"));
    }
    app.dispatch(Intent::RefreshMail).await;
    assert!(
        flat_subjects(&app.mailbox_list())
            .iter()
            .any(|s| s == "Budget"),
        "a later pass must reach the folder too: {:?}",
        flat_subjects(&app.mailbox_list()),
    );
}

#[tokio::test]
async fn opening_a_folder_a_pass_has_synced_downloads_nothing_more() {
    // An untagged folder bound when the account connected. The pass syncs it, so opening it
    // afterwards is only a matter of showing it: no connection of its own and no download.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = FlakyConnector::new("projects", Vec::new(), 0);
    let attempts = connector.attempts();
    let app = app_with_connector(
        vec![account_with(
            "acct-1",
            vec![
                FakeProvider::imap_inbox(Vec::new(), &["projects"]),
                FakeProvider::folder("projects", vec![message("p1", "projects", "Kick-off")]),
            ],
        )],
        connector,
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(open_folder("acct-1", "projects")).await;

    assert_eq!(flat_subjects(&app.mailbox_list()), vec!["Kick-off"]);
    assert_eq!(
        *attempts.lock().unwrap(),
        0,
        "a folder the pass synced was connected again when it was opened",
    );
}

#[tokio::test]
async fn an_account_wide_scope_covers_every_folder_it_lists() {
    // JMAP and Gmail sync the whole account through one scope, so no folder of theirs is
    // downloaded on its own: connecting one re-syncs the whole account.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = FlakyConnector::new("archive", Vec::new(), 0);
    let attempts = connector.attempts();
    let app = app_with_connector(
        vec![account("acct-1", FakeProvider::new())],
        connector,
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(open_folder("acct-1", "archive")).await;
    app.dispatch(Intent::RefreshMail).await;

    assert_eq!(*attempts.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_bound_folder_opened_before_any_pass_is_left_to_the_pass() {
    // At launch, and for as long as the device is offline, no pass has run yet. A folder the
    // account is bound to is the pass's to sync: opening it connects nothing (the connector here
    // would fail every time, as it does offline), and the list may say the folder is empty
    // rather than waiting for a download that is not its own (`docs/folder-pane.md`, rule 20).
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = FlakyConnector::new("projects", Vec::new(), u32::MAX);
    let attempts = connector.attempts();
    let app = app_with_connector(
        vec![account_with(
            "acct-1",
            vec![
                FakeProvider::imap_inbox(Vec::new(), &["projects"]),
                FakeProvider::folder("projects", Vec::new()),
            ],
        )],
        connector,
        &surfaces,
    );

    app.dispatch(open_folder("acct-1", "projects")).await;

    assert_eq!(*attempts.lock().unwrap(), 0);
    assert!(app.mailbox_list().empty_reason.is_some());
}

#[tokio::test]
async fn a_folder_the_pass_synced_empty_says_why_at_once() {
    // A synced folder with nothing in it has been looked at, so the list may say it is empty
    // (`docs/folder-pane.md`, rule 20) without waiting for a download that is not coming.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app_with_connector(
        vec![account_with(
            "acct-1",
            vec![
                FakeProvider::imap_inbox(Vec::new(), &["projects"]),
                FakeProvider::folder("projects", Vec::new()),
            ],
        )],
        FlakyConnector::new("projects", Vec::new(), 0),
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(open_folder("acct-1", "projects")).await;

    assert!(app.mailbox_list().empty_reason.is_some());
}

#[tokio::test]
async fn a_folder_that_only_holds_folders_is_never_connected_and_reads_as_empty() {
    // An IMAP `\Noselect` level, such as Gmail's `[Gmail]`: selecting it fails, so neither a pass
    // nor opening it may try. Opening it says it holds no mail, and offers no sync depth, which
    // would claim older mail is waiting on the server.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let connector = FlakyConnector::new("parent", Vec::new(), 0);
    let attempts = connector.attempts();
    let app = app_with_connector(
        vec![account(
            "acct-1",
            FakeProvider::imap_inbox(Vec::new(), &["parent"]).with_container("parent"),
        )],
        connector,
        &surfaces,
    );

    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(open_folder("acct-1", "parent")).await;
    app.dispatch(open_folder("acct-1", "parent")).await;

    assert_eq!(*attempts.lock().unwrap(), 0, "a container was connected");
    assert_eq!(app.mailbox_list().empty_reason, Some(EmptyReason::NoMail));
}
