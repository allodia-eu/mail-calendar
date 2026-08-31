//! The switcher's order: the sequence [`App::query_accounts`] reports, which every client renders
//! verbatim: the sidebar, the Settings cards, the search scope filter, the MCP account list.
//!
//! The order is the host's, not ours: each platform's secure store keeps an ordered account index,
//! and the core is handed those configs in that order at boot. What the core owes back is that it
//! does not *change* it, which is the whole of this file, because the one operation that used to
//! change it is the one every interactive launch performs on every account.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// The emails of the configured accounts, in switcher order.
async fn switcher(app: &crate::App<FakeProvider>) -> Vec<String> {
    app.query_accounts()
        .await
        .into_iter()
        .map(|row| row.email)
        .collect()
}

#[tokio::test]
async fn reconnecting_an_account_leaves_the_switcher_order_alone() {
    // Interactive boot lists provider-less placeholders in the host's stored order, then dials them
    // all in the background at most three at a time and replaces each with live providers as its
    // dial lands: so replacements arrive in network-latency order, not account order. Re-adding an
    // account must therefore replace it WHERE IT IS. Appending it instead re-sorted the whole
    // switcher into dial-completion order, once per account, live on screen.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
            account("acct-3", FakeProvider::new()),
        ],
        &surfaces,
    );
    let order = switcher(&app).await;

    // The middle one comes back first, then the last, then the first: any order at all.
    for id in ["acct-2", "acct-3", "acct-1"] {
        app.add_account_deferred(account(id, FakeProvider::with_idle(vec![])))
            .await;
        assert_eq!(
            switcher(&app).await,
            order,
            "reconnecting {id} moved the switcher"
        );
    }
}

#[tokio::test]
async fn reconnecting_an_account_leaves_the_switcher_order_alone_when_it_syncs() {
    // `add_account` is the same replacement with a sync attached (the OAuth completion paths and
    // the single-account add take it), so it owes the switcher the same thing.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
        ],
        &surfaces,
    );
    let order = switcher(&app).await;

    app.add_account(account("acct-1", FakeProvider::new()))
        .await;

    assert_eq!(switcher(&app).await, order);
}

#[tokio::test]
async fn a_genuinely_new_account_joins_at_the_end() {
    // The other half of the rule, and the reason this is a replacement rather than a sort: an
    // account the user has just added belongs last, matching the host store's ordered index, which
    // appends on first add and leaves every existing entry where it was.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);

    app.add_account_deferred(account("acct-2", FakeProvider::new()))
        .await;
    app.add_account_deferred(account("acct-1", FakeProvider::new()))
        .await;

    assert_eq!(
        switcher(&app).await,
        vec!["me@acct-1.local".to_owned(), "me@acct-2.local".to_owned()],
    );
}

#[tokio::test]
async fn removing_an_account_leaves_the_others_where_they_were() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account("acct-1", FakeProvider::new()),
            account("acct-2", FakeProvider::new()),
            account("acct-3", FakeProvider::new()),
        ],
        &surfaces,
    );

    app.remove_account(&engine_api::AccountId::try_from("acct-2").unwrap())
        .await;

    assert_eq!(
        switcher(&app).await,
        vec!["me@acct-1.local".to_owned(), "me@acct-3.local".to_owned()],
    );
}
