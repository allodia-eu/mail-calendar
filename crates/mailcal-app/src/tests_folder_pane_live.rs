//! An account's folders are on the pane once its pass has read the folder list, not once the
//! pass has downloaded every folder's mail.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::AccountId;

use crate::{
    Surface,
    tests_fakes::{FakeProvider, account, app, message},
};

fn published(surfaces: &Mutex<Vec<Surface>>) -> usize {
    surfaces
        .lock()
        .unwrap()
        .iter()
        .filter(|surface| **surface == Surface::MailboxList)
        .count()
}

fn shown_folders(app: &crate::App<FakeProvider>, id: &str) -> Vec<String> {
    app.mailbox_list()
        .account_folders
        .iter()
        .find(|row| row.account_id == id)
        .map(|row| {
            row.folders
                .iter()
                .map(|folder| folder.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_new_account_s_folders_show_while_its_mail_is_still_downloading() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let (provider, after_commit, finish) =
        FakeProvider::blocking(vec![message("m1", "a", "First")]);
    let app = Arc::new(app(vec![account("acct-1", provider)], &surfaces));
    let id = AccountId::try_from("acct-1").unwrap();
    assert!(
        shown_folders(&app, "acct-1").is_empty(),
        "nothing synced yet"
    );

    // The pass reads the folder list, commits the first page of mail and is then held open,
    // like a large folder still downloading.
    let pass = tokio::spawn({
        let app = Arc::clone(&app);
        let id = id.clone();
        async move { app.refresh_account(&id).await }
    });
    after_commit.notified().await;

    let shown = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let shown = shown_folders(&app, "acct-1");
            if !shown.is_empty() {
                return shown;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the folder list is drawn before the pass ends");
    assert_eq!(shown, ["a"]);
    assert!(!pass.is_finished(), "and the pass is still running");

    finish.notify_one();
    pass.await.unwrap();
}

#[tokio::test]
async fn a_pass_over_folders_already_shown_draws_nothing_extra() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "acct-1",
            FakeProvider::with(vec![message("m1", "a", "First")]),
        )],
        &surfaces,
    );
    let id = AccountId::try_from("acct-1").unwrap();
    app.refresh_account(&id).await;
    assert_eq!(shown_folders(&app, "acct-1"), ["a"]);

    let before = published(&surfaces);
    app.refresh_account(&id).await;
    assert_eq!(
        published(&surfaces) - before,
        1,
        "only the rebuild at the end of the pass: the tree did not change"
    );
}
