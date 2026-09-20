//! Keeping the message being composed on the server, at the layer a composer drives it from.
//!
//! The rules under test are in `docs/drafts.md`. The one worth stating twice: the stored copy
//! is named by the key the provider answered with, and three of the four real adapters move
//! that key on every save. `SubmitProvider` moves it too, which is what lets these tests tell
//! a correct save from one quietly storing a second copy each time.
//!
//! A child of [`super`] (the send tests), reusing its `SubmitProvider` and app builders; its
//! own file to keep each test module under the 500-line limit.

use std::sync::Arc;

use engine_api::ProviderKey;
use mailcal_composer::{Block, ComposerDocument, InlineContent, Paragraph, TextRun};
use tokio::sync::Notify;

use super::{SubmitProvider, app_over};
use crate::{CompositionId, DraftStatus, DraftsIntent, Intent};

fn composition(id: &str) -> CompositionId {
    CompositionId::new(id).expect("a composition id")
}

/// A one-paragraph document carrying `text`.
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
            })],
        })],
        attachments: Vec::new(),
    }
}

/// What a composer dispatches when its idle timer fires, or its Save button is pressed. The
/// core cannot tell those apart, and nothing here should.
fn save(id: &str, text: &str) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition(id),
        from: None,
        to: "you@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Later".to_owned(),
        document: document(text),
        blobs: Vec::new(),
    })
}

/// **The contract.** A re-save names the copy it supersedes, using the key the *previous* save
/// answered with.
///
/// The regression this exists for is silent and provider-specific. A caller that matched the
/// stored copy on the `Message-ID` would work on IMAP, JMAP and Graph and would store a second
/// draft on every save against Gmail, which keeps its draft object and rewrites the header
/// instead. So the assertion is on `replacing` carrying the first save's key, not merely on
/// there being two saves.
#[tokio::test]
async fn a_re_save_names_the_copy_it_supersedes() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "first")).await;
    app.dispatch(save("compose-1", "second")).await;

    let puts = puts.lock().unwrap();
    assert_eq!(puts.len(), 2, "each save stores the draft");
    assert_eq!(puts[0].1, None, "the first save supersedes nothing");
    assert_eq!(
        puts[1].1,
        Some(ProviderKey::new("draft-1").unwrap()),
        "the second save must name what the first one stored, or the server keeps both"
    );
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Saved
    );
}

/// A save whose content is unchanged reaches no server, and is still honestly "saved".
#[tokio::test]
async fn an_unchanged_save_reaches_no_server() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "same")).await;
    app.dispatch(save("compose-1", "same")).await;

    assert_eq!(
        puts.lock().unwrap().len(),
        1,
        "an idle timer firing on a composer nobody touched must not write"
    );
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Saved,
        "the words are on the server, which is what the composer should say"
    );
}

/// A save with no network is **queued**, not failed: the words are not lost.
///
/// `Failed` here would be the draft equivalent of telling someone a queued send failed, and it
/// invites the same reaction: writing the message again.
#[tokio::test]
async fn a_save_with_no_network_is_queued_rather_than_failed() {
    let provider = SubmitProvider::saving_nothing_for(1);
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "on a train")).await;

    assert!(puts.lock().unwrap().is_empty(), "the save had no network");
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued
    );
}

/// Two composers open at once save to their own stored copies.
///
/// This is what the composition id is for. Without it both would share one record, and each
/// save would name the *other* composer's key as the copy it supersedes: the second composer's
/// save would replace the first composer's draft on the server.
#[tokio::test]
async fn two_composers_supersede_only_their_own_draft() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "one")).await;
    app.dispatch(save("compose-2", "two")).await;
    app.dispatch(save("compose-1", "one again")).await;

    let puts = puts.lock().unwrap();
    assert_eq!(puts.len(), 3);
    assert_eq!(
        puts[1].1, None,
        "the second composer's first save is its own"
    );
    assert_eq!(
        puts[2].1,
        Some(ProviderKey::new("draft-1").unwrap()),
        "the first composer's re-save must name its own stored copy, not the other's"
    );
}

/// Discarding removes the copy the last save stored.
#[tokio::test]
async fn a_discard_removes_the_stored_copy() {
    let provider = SubmitProvider::new();
    let deletes = provider.draft_deletes();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "never mind")).await;
    app.dispatch(save("compose-1", "really never mind")).await;
    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("compose-1"),
    }))
    .await;

    assert_eq!(
        *deletes.lock().unwrap(),
        vec![ProviderKey::new("draft-2").unwrap()],
        "the removal must name the copy that is actually there, which the last save moved"
    );
}

/// Discarding withdraws a save that is still waiting for a network.
///
/// The queued op is the one copy of the message the composer is gone from. Left in the outbox
/// it drains after the discard and stores the very draft the user threw away, under a key no
/// composition holds any more, so nothing will ever remove it.
#[tokio::test]
async fn a_discard_withdraws_a_save_still_waiting_for_a_network() {
    let provider = SubmitProvider::saving_nothing_for(1);
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(save("compose-1", "forget it")).await;
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued
    );

    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("compose-1"),
    }))
    .await;
    app.dispatch(Intent::ReportNetworkReachable(true)).await;

    assert!(
        puts.lock().unwrap().is_empty(),
        "the drain must not store a draft the user discarded"
    );
}

/// The second of two overlapping saves supersedes what the first one stored.
///
/// A composer has two triggers, the idle timer and the Save button, and nothing stops both
/// firing; each is its own task. The second reads `replacing` before the first has recorded
/// what it stored, so without the exclusion it puts with nothing to replace and the server is
/// left holding two copies of the message still being written.
///
/// The engine keeps the two provider calls themselves from overlapping, since both saves of
/// one composition share a resource key and an op leases it. That is precisely why this fails
/// quietly rather than loudly, and why the assertion below is on `replacing` rather than on
/// how many calls were in flight at once.
#[tokio::test]
async fn two_overlapping_saves_store_one_draft() {
    // The provider parks in `put_draft`, so the first save is demonstrably still in flight
    // when the second is dispatched. Without that seam the two run to completion one after
    // the other whatever the code does, and the test proves nothing.
    let gate = Arc::new(Notify::new());
    let provider = SubmitProvider::saving_until(&gate);
    let puts = provider.draft_puts();
    let app = app_over(provider);

    let first = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(save("compose-1", "at once")).await }
    });
    while puts.lock().unwrap().is_empty() {
        tokio::task::yield_now().await;
    }
    let second = tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.dispatch(save("compose-1", "and again")).await }
    });
    // Let the second save get as far as it can while the first is still parked. Without
    // this it would start only after the first had finished and recorded its key, which is
    // the one ordering that cannot show the bug.
    for _ in 0..2048 {
        tokio::task::yield_now().await;
    }
    gate.notify_one();
    first.await.unwrap();
    gate.notify_one();
    second.await.unwrap();

    let puts = puts.lock().unwrap();
    assert_eq!(puts.len(), 2);
    assert_eq!(
        puts[1].1,
        Some(ProviderKey::new("draft-1").unwrap()),
        "the second save must supersede the copy the first one stored"
    );
}

/// Discarding a composition that was never saved reaches no server: there is nothing there.
#[tokio::test]
async fn a_discard_of_something_never_saved_reaches_no_server() {
    let provider = SubmitProvider::new();
    let deletes = provider.draft_deletes();
    let app = app_over(provider);

    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("compose-1"),
    }))
    .await;

    assert!(deletes.lock().unwrap().is_empty());
}

/// Closing a composer leaves the stored draft alone, and forgets the composition.
///
/// The second half is what stops a host reusing a composition id from superseding a draft the
/// user had finished with: the save after the close is a first save again.
#[tokio::test]
async fn closing_a_composer_keeps_the_draft_and_forgets_the_composition() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let deletes = provider.draft_deletes();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "keep this")).await;
    app.dispatch(Intent::Drafts(DraftsIntent::Close {
        composition: composition("compose-1"),
    }))
    .await;
    app.dispatch(save("compose-1", "a different message")).await;

    assert!(
        deletes.lock().unwrap().is_empty(),
        "closing a composer must not remove the draft it saved"
    );
    let puts = puts.lock().unwrap();
    assert_eq!(puts.len(), 2);
    assert_eq!(
        puts[1].1, None,
        "a reused id must start a new draft, not supersede the closed one"
    );
}

/// Every save of one composition carries the same `Message-ID`.
///
/// Not cosmetic: the engine keys a queued save's resource on that header, so a fresh one per
/// save would leave five offline saves queued beside each other instead of one superseding the
/// last, and the user's Drafts folder would collect all five when the network returned.
#[tokio::test]
async fn every_save_of_one_composition_carries_one_message_id() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(save("compose-1", "first")).await;
    app.dispatch(save("compose-1", "second")).await;
    app.dispatch(save("compose-2", "elsewhere")).await;

    let puts = puts.lock().unwrap();
    assert_eq!(
        puts[0].0.message_id, puts[1].0.message_id,
        "two saves of one composition are the same message"
    );
    assert_ne!(
        puts[0].0.message_id, puts[2].0.message_id,
        "a different composition is a different message"
    );
}

/// A draft may have no recipient yet. Refusing to keep one would refuse the drafts most worth
/// keeping: the ones the user has not finished addressing.
#[tokio::test]
async fn a_draft_with_no_recipient_is_still_saved() {
    let provider = SubmitProvider::new();
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(Intent::Drafts(DraftsIntent::Save {
        composition: composition("compose-1"),
        from: None,
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        subject: String::new(),
        document: document("who was this for again"),
        blobs: Vec::new(),
    }))
    .await;

    assert_eq!(puts.lock().unwrap().len(), 1);
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Saved
    );
}

/// **The hole a queued save opens.** A save made with no network is stored by a later drain,
/// and the composer is still open: the next save must supersede that stored copy.
///
/// Nothing else can tell it what the copy is called. The queued op settles with nobody
/// watching, and a settled op is not in the queue read, so the drain report is the only place
/// its key is ever named. Without that hand-back the composer saves again with nothing to
/// replace, and the user's Drafts folder collects a second copy of the message they are still
/// writing: the ordinary outcome of composing on a train.
#[tokio::test]
async fn a_save_stored_by_a_drain_is_what_the_next_save_supersedes() {
    // One refusal: the save queues, and the drain after it gets through.
    let provider = SubmitProvider::saving_nothing_for(1);
    let puts = provider.draft_puts();
    let app = app_over(provider);

    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    app.dispatch(save("compose-1", "somewhere with no signal"))
        .await;
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued
    );
    assert!(puts.lock().unwrap().is_empty());

    // Coming back online is what drains the queue, and it is how this happens for real: the
    // engine holds no timer for the outbox, so the reconnect is the wake signal.
    app.dispatch(Intent::ReportNetworkReachable(true)).await;
    assert_eq!(
        puts.lock().unwrap().len(),
        1,
        "the queued save never reached the server"
    );

    app.dispatch(save("compose-1", "back in signal")).await;

    let puts = puts.lock().unwrap();
    assert_eq!(puts.len(), 2);
    assert_eq!(
        puts[1].1,
        Some(ProviderKey::new("draft-1").unwrap()),
        "the save after the drain must name the copy the drain stored"
    );
}

/// A composer's hint is its own, and a closed one leaves none behind.
///
/// The rule the reading windows follow (`docs/reading-window.md`): a `DraftStatus` signal says
/// some composition's save moved, not which, so a composer that has saved nothing reads `Idle`
/// rather than the state of the one beside it. One slot for the app would have an untouched
/// composer announce that its neighbour had saved.
#[tokio::test]
async fn a_composers_hint_is_its_own() {
    let app = app_over(SubmitProvider::new());

    app.dispatch(save("compose-1", "mine")).await;

    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Saved
    );
    assert_eq!(
        app.draft_status(&composition("compose-2")),
        DraftStatus::Idle,
        "a composer that has saved nothing must not render its neighbour's hint"
    );

    // And a host reusing the id must not open on the last composer's answer.
    app.dispatch(Intent::Drafts(DraftsIntent::Close {
        composition: composition("compose-1"),
    }))
    .await;
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Idle
    );
}

/// A save whose text matches the last one that *landed* is still written while an earlier save
/// is queued, because the queued one carries different words.
///
/// The sequence is an ordinary undo on a train. The draft is saved, the network goes, an edit
/// queues, and the user undoes back to the text that is on the server. Comparing only against
/// the last landed digest reports "saved" and writes nothing, while the queued op is on its way
/// to replace the server's copy with the version the user just undid. The user is told their
/// current text is stored at the moment it is about to stop being.
#[tokio::test]
async fn an_unchanged_save_still_writes_while_an_earlier_one_is_queued() {
    // The first save lands; every one after it meets no network.
    let provider = SubmitProvider::saving_nothing_after(1);
    let app = app_over(provider);

    app.dispatch(save("compose-1", "the text")).await;
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Saved
    );

    app.dispatch(save("compose-1", "an edit")).await;
    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued
    );

    // Undone back to exactly what is on the server. The digest matches the last landed save,
    // and it must not be believed: the queued op still carries "an edit".
    app.dispatch(save("compose-1", "the text")).await;

    assert_eq!(
        app.draft_status(&composition("compose-1")),
        DraftStatus::Queued,
        "a composition with a queued save must not report the server as up to date"
    );
}
