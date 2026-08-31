//! Reading-open behaviour tests for [`super::App::open_message`] beyond body content: the
//! mark-as-read-on-open side effect and the cold-open resilience that waits for an account's
//! mail provider to finish dialing (the notification-tap race). Body sanitise/inline-image/
//! attachment/recipient coverage lives in `tests_actions.rs`. Shared fixtures: `tests_fakes.rs`.
//!
//! Also here: *when* an open is allowed to say it is still working, which is the difference
//! between a fast app and a flickering one.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use engine_api::{AccountId, EmailAddress};
use engine_provider::MailEdit;
use fakes::{FakeProvider, account, app, message, msg};

use super::{Intent, Surface};
use crate::Account;

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

#[tokio::test]
async fn opening_an_unread_message_marks_it_read_on_the_server() {
    use engine_core::mail::{Keyword, SystemKeyword};

    let provider = FakeProvider::new();
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await; // m1 (unread in the fixture) into the store

    // Opening the message both publishes the reading view and marks it Seen on the server, so the
    // read state reflects that the user has now seen it: no separate mark-read action needed.
    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(
        edits.len(),
        1,
        "opening an unread message forwards exactly one Seen edit"
    );
    match &edits[0] {
        MailEdit::SetKeywords { target, add, .. } => {
            assert_eq!(target.as_str(), "m1");
            assert!(add.contains(&Keyword::system(SystemKeyword::Seen)));
        }
        other => panic!("expected a SetKeywords edit, got {other:?}"),
    }
}

#[tokio::test]
async fn opening_an_already_read_message_does_not_re_mark_it() {
    use engine_core::mail::{Keyword, SystemKeyword};

    // A message the server already has flagged Seen: opening it must not forward a redundant
    // mark-read edit (which would re-sync every account on every open of a read message).
    let mut already_read = message("m1", "a", "Already read");
    already_read
        .keywords
        .insert(Keyword::system(SystemKeyword::Seen));
    let provider = FakeProvider::with(vec![already_read]);
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    assert!(
        !app.reading_view().load_error,
        "the body should load so the read-state check runs on a real open"
    );
    assert!(
        edits.lock().unwrap().is_empty(),
        "an already-read message triggers no mark-read edit"
    );
}

#[tokio::test]
async fn open_message_waits_for_a_still_dialing_account_then_loads() {
    // The cold-open-from-notification race: the message is already in the store (a background
    // pass synced it), but the account's mail provider dials asynchronously after boot, so the
    // first body fetch finds no provider. Opening must keep spinning and retry, not fail outright.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = Arc::new(app(vec![account("acct-1", FakeProvider::new())], &surfaces));
    app.dispatch(Intent::RefreshMail).await; // m1 is now in the store

    // Reconnect the account as a provider-less placeholder: the "still dialing" boot state.
    app.add_account(Account {
        id: AccountId::try_from("acct-1").unwrap(),
        providers: Vec::new(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@acct-1.local"),
    })
    .await;

    let open = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.dispatch(Intent::OpenMessage {
                message: msg("acct-1", "m1"),
            })
            .await;
        }
    });
    // While the account has no provider the open must keep waiting, not resolve to an error.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !open.is_finished(),
        "the open must keep waiting while the account is still dialing"
    );

    // The dial completes: reconnect the account WITH a provider (the message is still in the
    // store).
    app.add_account(account("acct-1", FakeProvider::new()))
        .await;

    // The next poll picks up the provider and the body loads: no load error.
    tokio::time::timeout(Duration::from_secs(3), open)
        .await
        .expect("the open should finish once the provider connects")
        .unwrap();
    let reading = app.reading_view();
    assert_eq!(reading.key, "m1");
    assert!(
        !reading.load_error,
        "the body should load once the provider connected"
    );
    assert!(reading.html.is_some() || reading.plain.is_some());
}

#[tokio::test(start_paused = true)]
async fn open_message_gives_up_after_the_dial_window_when_no_provider_connects() {
    // The bound on the retry above: an account that never finishes dialing must not hang the open
    //; it surfaces the load error once the wait window elapses. The paused clock auto-advances
    // through the bounded polls, so the test settles instantly instead of waiting the real window.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    app.dispatch(Intent::RefreshMail).await; // m1 is in the store

    // Drop to a provider-less placeholder that never reconnects.
    app.add_account(Account {
        id: AccountId::try_from("acct-1").unwrap(),
        providers: Vec::new(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@acct-1.local"),
    })
    .await;

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;
    let reading = app.reading_view();
    assert_eq!(reading.key, "m1");
    assert!(
        reading.load_error,
        "a never-connecting account must surface the load error, not hang"
    );
}

#[tokio::test]
async fn a_fast_open_never_announces_a_wait() {
    // Moving between messages is the common case and a stored body comes back in milliseconds.
    // Announcing it anyway put a spinner on screen and removed it within the same eyeblink, on
    // every platform: one publish, carrying the body, and nothing before it.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", FakeProvider::new())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    surfaces.lock().unwrap().clear();

    app.dispatch(Intent::OpenMessage {
        message: msg("acct-1", "m1"),
    })
    .await;

    let reading = app.reading_view();
    assert_eq!(reading.key, "m1");
    assert!(
        !reading.pending,
        "a body that arrived is not a wait to announce"
    );
    assert!(reading.html.is_some() || reading.plain.is_some());
    let announced = surfaces
        .lock()
        .unwrap()
        .iter()
        .filter(|surface| **surface == Surface::Reading)
        .count();
    assert_eq!(
        announced, 1,
        "a fast open must publish once (the body) and never a spinner before it"
    );
}

#[tokio::test(start_paused = true)]
async fn an_open_that_outlasts_the_threshold_announces_the_wait_first() {
    // The other half: a wait long enough to notice must not be silent. Same cold-open race as
    // above: the account is still dialing, so the body cannot arrive yet.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = Arc::new(app(vec![account("acct-1", FakeProvider::new())], &surfaces));
    app.dispatch(Intent::RefreshMail).await;
    app.add_account(Account {
        id: AccountId::try_from("acct-1").unwrap(),
        providers: Vec::new(),
        calendar_providers: Vec::new(),
        contact_providers: Vec::new(),
        identity: EmailAddress::new("me@acct-1.local"),
    })
    .await;

    let open = tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            app.dispatch(Intent::OpenMessage {
                message: msg("acct-1", "m1"),
            })
            .await;
        }
    });
    // Past the threshold, with the body still unresolved.
    tokio::time::sleep(Duration::from_millis(700)).await;
    let waiting = app.reading_view();
    assert!(
        waiting.pending,
        "an open still running past the threshold must say so"
    );
    assert_eq!(waiting.key, "m1", "and say which message it is waiting on");
    assert!(
        waiting.html.is_none() && waiting.plain.is_none(),
        "the announcement carries no body"
    );

    // The dial completes; the body lands and supersedes the announcement.
    app.add_account(account("acct-1", FakeProvider::new()))
        .await;
    tokio::time::timeout(Duration::from_secs(3), open)
        .await
        .expect("the open finishes once the provider connects")
        .unwrap();
    let loaded = app.reading_view();
    assert!(!loaded.pending, "the wait is over");
    assert!(loaded.html.is_some() || loaded.plain.is_some());
}
