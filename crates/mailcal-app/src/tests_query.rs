//! The two **guarantee tests** for the query API, plus its paging and scope behaviour.
//!
//! The guarantees are the highest-value tests in the agent surface, and neither is obvious from
//! reading the code: `query_message` and `App::open_message` differ by one call, and a query
//! looks so much like the intent it replaces that "simplify this into a dispatch" is a natural
//! and completely wrong refactor. These fail loudly when someone tries it.

use std::sync::{Arc, Mutex};

use engine_api::{AccountId, Message, UtcDateTime};
use engine_core::mail::{Keyword, SystemKeyword};
use engine_provider::MailEdit;
use fakes::{FakeProvider, account, app, message, msg, open_folder};

use crate::{Intent, SearchScope};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Every read the agent surface can perform, run against one fixture.
async fn run_every_read(app: &crate::App<FakeProvider>, account_id: &AccountId) {
    app.query_accounts().await;
    app.query_folders(account_id).await;
    app.query_folder_page(account_id, Some("a"), false, 0, 20)
        .await;
    app.query_folder_page(account_id, None, true, 0, 20).await;
    app.query_search("report", None, None, 0, 20).await;
    app.query_search("report", Some(account_id), Some("a"), 0, 20)
        .await;
    app.query_message(&msg(account_id.as_str(), "m1")).await;
}

#[tokio::test]
async fn reads_never_mutate_the_server() {
    // GUARANTEE 1. `Intent::OpenMessage` marks a message read ON THE SERVER, so an assistant
    // answering "read me that email" over the intent path would silently clear the unread badge
    // on the user's real mailbox: an irreversible side effect of a question. Every read here
    // must leave the provider untouched.
    let provider = FakeProvider::with(vec![message("m1", "a", "Quarterly report")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    let account_id = AccountId::try_from("acct-1").unwrap();

    run_every_read(&app, &account_id).await;

    assert!(
        edits.lock().unwrap().is_empty(),
        "no read reached the provider as an edit: {:?}",
        edits.lock().unwrap(),
    );
    assert!(
        app.query_message(&msg("acct-1", "m1"))
            .await
            .expect("the message resolves")
            .unread,
        "and the message is still unread after being read in full",
    );
}

#[tokio::test]
async fn the_intent_path_by_contrast_does_mark_it_read() {
    // The paired contrast, in the same file on purpose: this states WHY `query_message` exists.
    // If this ever stops recording a `$seen` write, `reads_never_mutate_the_server` has become a
    // test of nothing: a check that cannot fail is not a check.
    let provider = FakeProvider::with(vec![message("m1", "a", "Quarterly report")]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1, "opening it wrote exactly one edit");
    assert!(
        matches!(&edits[0], MailEdit::SetKeywords { add, .. } if !add.is_empty()),
        "and that edit marks it seen: {:?}",
        edits[0],
    );
}

#[tokio::test]
async fn reads_never_move_the_users_screen() {
    // GUARANTEE 2. Every read-shaped Intent ends in `rebuild_snapshot()`, and `Search` also
    // rewrites the active query and scope. An assistant listing a folder or running a search
    // must not scroll, re-scope or re-filter the list of a person who is reading something else.
    let provider = FakeProvider::with_trash(vec![
        message("m1", "a", "Quarterly report"),
        message("m2", "a", "Lunch plans"),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    // Put the user somewhere specific and narrow: one account, one folder, an active search
    // that has been scoped down, and a window they have scrolled.
    app.dispatch(Intent::SelectAccount(Some("acct-1".to_owned())))
        .await;
    app.dispatch(open_folder("acct-1", "a")).await;
    app.dispatch(Intent::Search(Some("lunch".to_owned()))).await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::ShowMore).await;

    let before = (
        app.mailbox_list(),
        app.scope.lock().unwrap().clone(),
        app.search_query.lock().unwrap().clone(),
        *app.search_scope.lock().unwrap(),
        app.visible_limit(),
    );
    surfaces.lock().unwrap().clear();

    let account_id = AccountId::try_from("acct-1").unwrap();
    run_every_read(&app, &account_id).await;

    let after = (
        app.mailbox_list(),
        app.scope.lock().unwrap().clone(),
        app.search_query.lock().unwrap().clone(),
        *app.search_scope.lock().unwrap(),
        app.visible_limit(),
    );
    assert_eq!(
        before.0, after.0,
        "the published mailbox list is byte-identical",
    );
    assert_eq!(
        before.1, after.1,
        "the account and folder on screen did not move",
    );
    assert_eq!(before.2, after.2, "the user's search query survived");
    assert_eq!(before.3, after.3, "and so did the scope they narrowed to");
    assert_eq!(before.4, after.4, "and the window they had scrolled");
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "no surface was signalled at all: the host was never told to re-pull: {:?}",
        surfaces.lock().unwrap(),
    );
}

#[tokio::test]
async fn a_folder_page_is_newest_first_and_pages_without_gaps_or_repeats() {
    let provider = FakeProvider::with(vec![
        dated("m1", "Oldest", 1),
        dated("m2", "Middle", 2),
        dated("m3", "Newest", 3),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    let account_id = AccountId::try_from("acct-1").unwrap();

    let first = app
        .query_folder_page(&account_id, Some("a"), false, 0, 2)
        .await;
    assert_eq!(subjects(&first), ["Newest", "Middle"]);
    assert_eq!(first.total, 3, "total is the whole folder, not the page");

    let second = app
        .query_folder_page(&account_id, Some("a"), false, 2, 2)
        .await;
    assert_eq!(subjects(&second), ["Oldest"]);
    assert_eq!(second.offset, 2, "the page echoes where it started");
}

#[tokio::test]
async fn unread_only_narrows_the_total_as_well_as_the_rows() {
    // A `total` that counted read mail too would make "you have 3 unread" a lie the moment a
    // caller trusted it.
    let mut read = message("m2", "a", "Already read");
    read.keywords.insert(Keyword::system(SystemKeyword::Seen));
    let provider = FakeProvider::with(vec![message("m1", "a", "Unread"), read]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    let account_id = AccountId::try_from("acct-1").unwrap();

    let page = app
        .query_folder_page(&account_id, Some("a"), true, 0, 20)
        .await;
    assert_eq!(subjects(&page), ["Unread"]);
    assert_eq!(page.total, 1);
}

#[tokio::test]
async fn a_search_defaults_to_every_folder_except_trash_and_reaches_it_when_narrowed() {
    // Rule 2 of docs/search.md, inherited rather than reimplemented: the default scope skips
    // Trash, and narrowing to Trash is how you search it.
    let provider = FakeProvider::with_trash(vec![
        message("m1", "a", "Invoice for March"),
        message("m2", "trash", "Invoice for February"),
    ]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    let account_id = AccountId::try_from("acct-1").unwrap();

    let wide = app.query_search("Invoice", None, None, 0, 20).await;
    assert_eq!(
        subjects(&wide),
        ["Invoice for March"],
        "the trashed invoice is out of the default scope",
    );

    let narrowed = app
        .query_search("Invoice", Some(&account_id), Some("trash"), 0, 20)
        .await;
    assert_eq!(
        subjects(&narrowed),
        ["Invoice for February"],
        "and reachable by naming the folder",
    );
}

#[tokio::test]
async fn a_read_does_not_warm_the_uis_list_cache() {
    // Paging deep as an agent must not leave the UI projecting from a four-thousand-row cache on
    // every later rebuild. The query reads the store directly for exactly this reason.
    let provider = FakeProvider::with(vec![message("m1", "a", "Hello")]);
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    let account_id = AccountId::try_from("acct-1").unwrap();
    app.dispatch(Intent::RefreshMail).await;
    app.invalidate_list_cache();

    app.query_folder_page(&account_id, Some("a"), false, 0, 20)
        .await;

    assert!(
        !app.list_cache_is_loaded(),
        "the query left the cache exactly as it found it; dropped, not repopulated",
    );
}

/// A message with a delivery instant `day` days into 2026, so ordering is deterministic.
fn dated(id: &str, subject: &str, day: u8) -> Message {
    let mut message = message(id, "a", subject);
    message.received_at = Some(UtcDateTime::new(2026, 1, day, 9, 0, 0).expect("a valid instant"));
    message
}

fn subjects(page: &crate::MessagePage) -> Vec<String> {
    page.rows.iter().map(|row| row.subject.clone()).collect()
}
