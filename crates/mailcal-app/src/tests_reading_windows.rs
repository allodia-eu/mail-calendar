//! Several readers of one core: the pane and the detached reading windows a desktop opens beside
//! it (`docs/reading-window.md`).
//!
//! What these hold down is that the slot is the only thing a window changes. Everything else about
//! an open, the mark-read, the retry, the pending threshold, is the pane's, reached through the
//! same call: so what is asserted here is that two readers do not collide, that closing one frees
//! what it held, and that the pane cannot be reached by naming a window.

use std::sync::{Arc, Mutex};

use fakes::{FakeProvider, account, app, message, msg};

use super::{Intent, ReaderId};

#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// A provider holding two readable messages, so two readers can be on different ones.
fn two_messages() -> FakeProvider {
    FakeProvider::with(vec![
        message("m1", "a", "The first"),
        message("m2", "a", "The second"),
    ])
}

#[tokio::test]
async fn a_window_and_the_pane_hold_different_messages_at_once() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", two_messages())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: msg("acct-1", "m1"),
    })
    .await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Window("w1".to_owned()),
        message: msg("acct-1", "m2"),
    })
    .await;

    // The whole point of the feature: opening a message in a window leaves the message the user
    // was reading in the pane exactly where it was.
    assert_eq!(app.reading_view().key, "m1");
    assert_eq!(
        app.reading_view_in(&ReaderId::Window("w1".to_owned())).key,
        "m2"
    );
}

#[tokio::test]
async fn two_windows_hold_their_own_message_each() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", two_messages())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Window("w1".to_owned()),
        message: msg("acct-1", "m1"),
    })
    .await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Window("w2".to_owned()),
        message: msg("acct-1", "m2"),
    })
    .await;

    assert_eq!(
        app.reading_view_in(&ReaderId::Window("w1".to_owned())).key,
        "m1"
    );
    assert_eq!(
        app.reading_view_in(&ReaderId::Window("w2".to_owned())).key,
        "m2"
    );
    // Nothing was opened in the pane, and no window's body leaked into it.
    assert!(app.reading_view().key.is_empty());
}

#[tokio::test]
async fn closing_a_window_frees_its_body_and_leaves_every_other_reader_alone() {
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", two_messages())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: msg("acct-1", "m1"),
    })
    .await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Window("w1".to_owned()),
        message: msg("acct-1", "m2"),
    })
    .await;

    app.close_reader(&ReaderId::Window("w1".to_owned()));

    // A closed window reads as the empty snapshot, which is also what one that has never opened
    // reads as: both mean "there is no body here".
    assert!(
        app.reading_view_in(&ReaderId::Window("w1".to_owned()))
            .key
            .is_empty()
    );
    assert_eq!(app.reading_view().key, "m1", "the pane keeps its message");
}

#[tokio::test]
async fn closing_a_window_cannot_empty_the_pane() {
    // A host mints window ids as strings, and the pane's slot is a different variant, so there is
    // no string a host can pass that reaches it. This is the test for that, because the failure it
    // guards against, the reading pane going blank when a window is closed, would look to a user
    // like the app losing their place.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", two_messages())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;
    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Pane,
        message: msg("acct-1", "m1"),
    })
    .await;

    for name in ["", "pane", "Pane", "main", "0"] {
        app.close_reader(&ReaderId::Window(name.to_owned()));
    }

    assert_eq!(app.reading_view().key, "m1");
}

#[tokio::test]
async fn opening_in_a_window_marks_the_message_read_as_the_pane_does() {
    use engine_api::SystemKeyword;
    use engine_provider::MailEdit;

    // The proof that a window is a reader and not a second open: the mark-read is the open's, so
    // it happens for a window without anything restating it (`crate::reading`).
    let provider = two_messages();
    let edits = provider.edits();
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", provider)], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    app.dispatch(Intent::OpenMessage {
        reader: ReaderId::Window("w1".to_owned()),
        message: msg("acct-1", "m1"),
    })
    .await;

    let edits = edits.lock().unwrap();
    assert_eq!(edits.len(), 1, "one Seen edit, as a pane open makes");
    match &edits[0] {
        MailEdit::SetKeywords { target, add, .. } => {
            assert_eq!(target.as_str(), "m1");
            assert!(add.contains(&engine_core::mail::Keyword::system(SystemKeyword::Seen)));
        }
        other => panic!("expected a SetKeywords edit, got {other:?}"),
    }
}

#[tokio::test]
async fn re_opening_the_same_window_replaces_what_it_held() {
    // A window is one slot for as long as it lives, not one per message: a client that reuses a
    // window for another message must not leave the first body behind it.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("acct-1", two_messages())], &surfaces);
    app.dispatch(Intent::RefreshMail).await;

    for key in ["m1", "m2"] {
        app.dispatch(Intent::OpenMessage {
            reader: ReaderId::Window("w1".to_owned()),
            message: msg("acct-1", key),
        })
        .await;
    }

    assert_eq!(
        app.reading_view_in(&ReaderId::Window("w1".to_owned())).key,
        "m2"
    );
}
