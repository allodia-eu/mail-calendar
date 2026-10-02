//! Leaving a composer: the save that also closes it, and the question a Discard asks first.
//!
//! The rules are in `docs/drafts.md` under "Leaving a composer". A child of [`super`] (the send
//! tests), reusing its `SubmitProvider` and app builders; its own file because
//! `draft_ops_tests.rs` is at the 500-line limit.

use engine_api::ProviderKey;
use mailcal_composer::{Block, ComposerDocument, InlineContent, Paragraph, TextRun};

use super::{SubmitProvider, app_over};
use crate::{CompositionId, DraftStatus, DraftsIntent, Intent};

fn composition(id: &str) -> CompositionId {
    CompositionId::new(id).expect("a composition id")
}

fn document(text: &str) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![InlineContent::Text(TextRun {
                text: text.to_owned(),
                bold: false,
                italic: false,
                underline: false,
                font_size: None,
                color: None,
                highlight: None,
                link: None,
            })],
        })],
        attachments: Vec::new(),
    }
}

/// A save, closing the composition after it when `then_close` is set.
fn save(id: &str, text: &str, then_close: bool) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition(id),
        from: None,
        to: "you@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Later".to_owned(),
        document: document(text),
        blobs: Vec::new(),
        then_close,
    })
}

/// Leaving a composer stores what it holds over its own stored copy, then forgets it.
///
/// One intent rather than a save and a close, because each intent is its own task: a close
/// that landed first would leave the save with no record, so it would supersede nothing and
/// the server would keep the old copy beside the new one.
#[tokio::test]
async fn leaving_a_composer_saves_over_its_stored_copy_and_forgets_it() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let deletes = provider.draft_deletes();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "first", false)).await;
    app.dispatch(save("compose-1", "typed after the last save", true))
        .await;

    {
        let puts = puts.lock().unwrap();
        assert_eq!(puts.len(), 2, "what was typed since the last save is kept");
        assert_eq!(
            puts[1].1,
            Some(ProviderKey::new("draft-1").unwrap()),
            "the leaving save supersedes the stored copy rather than adding a second"
        );
    }
    assert!(
        deletes.lock().unwrap().is_empty(),
        "leaving never removes the draft"
    );
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Idle,
        "the composition is forgotten, hint and all"
    );
    assert!(!app.draft_is_stored(&composition("compose-1")));

    app.dispatch(save("compose-1", "a different message", false))
        .await;
    assert_eq!(
        puts.lock().unwrap()[2].1,
        None,
        "a reused id starts a new draft, not a save over the one left in Drafts"
    );
}

/// Leaving a composer whose words are already on the server writes nothing.
///
/// The ordinary case: the idle timer saved a moment ago and the user clicks another message.
#[tokio::test]
async fn leaving_an_unchanged_composer_writes_nothing() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "already saved", false))
        .await;
    app.dispatch(save("compose-1", "already saved", true)).await;

    assert_eq!(puts.lock().unwrap().len(), 1);
    assert!(!app.draft_is_stored(&composition("compose-1")));
}

/// A composition reports a stored copy once a save reached the server or is queued for one,
/// and stops when it is discarded.
///
/// What a composer's Discard asks before it removes anything: with no stored copy and nothing
/// typed there is nothing to lose, and no question to ask.
#[tokio::test]
async fn a_composition_says_whether_it_has_a_stored_copy() {
    let app = app_over(SubmitProvider::new());
    assert!(!app.draft_is_stored(&composition("compose-1")));

    app.dispatch(save("compose-1", "on the server", false))
        .await;
    assert!(app.draft_is_stored(&composition("compose-1")));

    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("compose-1"),
    }))
    .await;
    assert!(!app.draft_is_stored(&composition("compose-1")));
}

/// A save queued for a network counts as stored: discarding is what withdraws it.
#[tokio::test]
async fn a_queued_save_counts_as_a_stored_copy() {
    let app = app_over(SubmitProvider::saving_nothing_for(1));

    app.dispatch(save("compose-1", "on a train", false)).await;

    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued
    );
    assert!(app.draft_is_stored(&composition("compose-1")));
}
