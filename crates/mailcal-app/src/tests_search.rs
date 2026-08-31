//! Search tests for [`super::App`]: the newest-first order across accounts, the Trash the
//! default scope leaves out, and what "current folder" means in each of the three shapes the
//! mailbox list takes (the unified inboxes, one account's all-mail, one folder). The shared
//! fixtures live in `tests_fakes.rs`.
//!
//! Every case searches for `Report`, which the fixtures put in the subject of the messages
//! that should match and leave out of the ones that should not: so a case reads as "which
//! of the matching messages does this scope show".

use std::sync::{Arc, Mutex};

use engine_core::mail::Message;
use fakes::{FakeProvider, account, app, flat_subjects, message, open_folder};
use mailcal_viewmodel::SearchHorizon;

use super::{Intent, SearchScope};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A message in `mailbox` delivered on `day` of June 2026: a distinct date per message, so
/// the newest-first order is observable rather than incidental.
fn dated(id: &str, mailbox: &str, subject: &str, day: u8) -> Message {
    let mut message = message(id, mailbox, subject);
    message.received_at = Some(format!("2026-06-{day:02}T09:00:00Z").parse().unwrap());
    message
}

#[tokio::test]
async fn search_orders_hits_newest_first_across_accounts() {
    // Two accounts, each holding an old and a recent match. The engine ranks by relevance
    // within an account and we merge by date, so the four hits must interleave strictly by
    // date; never "all of account A, then all of account B".
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![
                    dated("w1", "a", "Report from work, oldest", 1),
                    dated("w2", "a", "Report from work, newest", 4),
                ]),
            ),
            account(
                "home",
                FakeProvider::with(vec![
                    dated("h1", "a", "Report from home, older", 2),
                    dated("h2", "a", "Report from home, newer", 3),
                ]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec![
            "Report from work, newest",
            "Report from home, newer",
            "Report from home, older",
            "Report from work, oldest",
        ],
    );
}

#[tokio::test]
async fn search_leaves_out_trashed_mail_by_default() {
    // A message the user threw away is not what they are looking for: the default scope skips
    // Trash, while the same query still finds the copy filed in Archive.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with_trash(vec![
                dated("m1", "a", "Report in the inbox", 1),
                dated("m2", "archive", "Report in the archive", 2),
                dated("m3", "trash", "Report in the trash", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the archive", "Report in the inbox"],
    );
}

#[tokio::test]
async fn searching_inside_trash_finds_trashed_mail() {
    // The Trash exclusion is a default, not a wall: standing in Trash and narrowing to the
    // current folder searches it; otherwise the folder would be unsearchable.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with_trash(vec![
                dated("m1", "a", "Report in the inbox", 1),
                dated("m3", "trash", "Report in the trash", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(open_folder("work", "trash")).await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the trash"]
    );
}

#[tokio::test]
async fn the_default_scope_searches_every_account_even_with_one_selected() {
    // Standing in one account's folder does not narrow the default scope: "all folders" means
    // every account's, so the other account's newer hit still leads.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![dated("w1", "a", "Report from work", 1)]),
            ),
            account(
                "home",
                FakeProvider::with(vec![dated("h1", "a", "Report from home", 2)]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    let snapshot = app.mailbox_list();
    assert_eq!(
        flat_subjects(&snapshot),
        vec!["Report from home", "Report from work"],
    );
    // …while the switcher stays on the account the user is in, so leaving search returns
    // there rather than to the unified view.
    assert_eq!(snapshot.selected_account.as_deref(), Some("work"));
}

#[tokio::test]
async fn the_current_folder_scope_narrows_to_the_selected_account_and_folder() {
    // The same two accounts, narrowed: only the selected account's selected folder answers.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with_trash(vec![
                    dated("w1", "a", "Report in the work inbox", 1),
                    dated("w2", "archive", "Report in the work archive", 2),
                ]),
            ),
            account(
                "home",
                FakeProvider::with(vec![dated("h1", "a", "Report from home", 3)]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(open_folder("work", "archive")).await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    let snapshot = app.mailbox_list();
    assert_eq!(flat_subjects(&snapshot), vec!["Report in the work archive"],);
    // The folder selection rides through the search, so the host keeps showing where it is
    // (and can name the "this folder" side of its filter).
    assert_eq!(snapshot.selected.as_deref(), Some("archive"));
}

#[tokio::test]
async fn the_current_folder_scope_in_the_unified_view_searches_every_inbox() {
    // "Current folder" has to mean something in the unified view too: there it is the set of
    // inboxes on screen; every account's, and nothing filed elsewhere.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with_trash(vec![
                    dated("w1", "a", "Report in the work inbox", 1),
                    dated("w2", "archive", "Report in the work archive", 4),
                ]),
            ),
            account(
                "home",
                FakeProvider::with(vec![dated("h1", "a", "Report in the home inbox", 2)]),
            ),
        ],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the home inbox", "Report in the work inbox"],
    );
}

#[tokio::test]
async fn the_current_folder_scope_over_an_account_searches_its_whole_mailbox() {
    // An account selected with no folder is its all-mail view, Trash included; "current
    // folder" mirrors the list, so it must not quietly apply the default's Trash exclusion.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with_trash(vec![
                dated("m1", "a", "Report in the inbox", 1),
                dated("m3", "trash", "Report in the trash", 3),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the trash", "Report in the inbox"],
    );
}

#[tokio::test]
async fn leaving_search_resets_the_scope_and_restores_the_folder_view() {
    // Two things at once, because they are the same moment: clearing the query drops the
    // narrowing (an invisible filter would silently shrink the next search), and the list
    // returns to the folder the user was in rather than staying on the results.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with_trash(vec![
                dated("m1", "a", "Report in the inbox", 1),
                dated("m2", "archive", "Report in the archive", 2),
            ]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(open_folder("work", "a")).await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;
    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the inbox"],
    );

    // Leaving search: the inbox is back, with its own (unsearched) contents.
    app.dispatch(Intent::Search(None)).await;
    let restored = app.mailbox_list();
    assert_eq!(flat_subjects(&restored), vec!["Report in the inbox"]);
    assert_eq!(restored.selected.as_deref(), Some("a"));
    assert_eq!(restored.selected_account.as_deref(), Some("work"));

    // …and the next search opens wide again, reaching the archive it had been narrowed off.
    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;
    assert_eq!(
        flat_subjects(&app.mailbox_list()),
        vec!["Report in the archive", "Report in the inbox"],
    );
}

#[tokio::test]
async fn search_states_how_far_back_it_looked_and_takes_the_narrowest_account() {
    // Search reads the local store, so it finds only what sync depth kept. Across accounts the
    // answer is as complete as the *least* complete one: a six-month account searched beside a
    // three-month one still cannot answer a question about last year.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![dated("w1", "a", "Report", 1)]),
            ),
            account(
                "home",
                FakeProvider::with(vec![dated("h1", "a", "Report", 2)]),
            ),
        ],
        &surfaces,
    );
    app.set_account_sync_depth("work", 6).await;
    app.set_account_sync_depth("home", 3).await;
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        app.mailbox_list().search_horizon,
        Some(SearchHorizon::Months(3)),
    );
}

#[tokio::test]
async fn a_narrowed_scope_states_the_depth_of_the_account_it_actually_searched() {
    // The horizon describes the accounts the scope covers, not every account configured.
    // Narrowed to the six-month account, "the last three months" would be a different, and
    // wrong; statement about a search the three-month account took no part in.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![
            account(
                "work",
                FakeProvider::with(vec![dated("w1", "a", "Report", 1)]),
            ),
            account(
                "home",
                FakeProvider::with(vec![dated("h1", "a", "Report", 2)]),
            ),
        ],
        &surfaces,
    );
    app.set_account_sync_depth("work", 6).await;
    app.set_account_sync_depth("home", 3).await;
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::SelectAccount(Some("work".to_owned())))
        .await;
    app.dispatch(Intent::SetSearchScope(SearchScope::CurrentFolder))
        .await;

    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        app.mailbox_list().search_horizon,
        Some(SearchHorizon::Months(6)),
    );
}

#[tokio::test]
async fn an_account_holding_all_its_mail_bounds_the_search_by_nothing() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![dated("w1", "a", "Report", 1)]),
        )],
        &surfaces,
    );
    app.set_account_sync_depth("work", 0).await;
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;

    assert_eq!(
        app.mailbox_list().search_horizon,
        Some(SearchHorizon::AllTime),
    );
}

#[tokio::test]
async fn a_list_that_is_not_a_search_states_no_horizon() {
    // The horizon qualifies an answer to a question. A folder the user opened asked nothing,
    // so a client keys the whole line off this field being present.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account(
            "work",
            FakeProvider::with(vec![dated("w1", "a", "Report", 1)]),
        )],
        &surfaces,
    );
    app.dispatch(Intent::RefreshMail).await;

    assert_eq!(app.mailbox_list().search_horizon, None);

    app.dispatch(Intent::Search(Some("Report".to_owned())))
        .await;
    assert!(app.mailbox_list().search_horizon.is_some());
    app.dispatch(Intent::Search(None)).await;
    assert_eq!(app.mailbox_list().search_horizon, None);
}
