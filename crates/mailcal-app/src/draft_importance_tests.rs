//! The importance a composition goes out with is the core's: picked once, carried by every save
//! and the send, and reopened with a stored draft (`docs/importance.md`).
//!
//! A child of [`super`] (the rich-submit tests), over [`super::mail_fake`]'s provider.

use engine_api::Importance;
use mailcal_composer::{Block, ComposerDocument, Paragraph};

use super::mail_fake::{
    ThreadProvider, app_and_logs, dispatch_until, rich_stored_draft, staging_dir,
};
use crate::{CompositionId, DraftsIntent, Intent, MessageRef, SendStatus};

fn empty_document() -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph::new(Vec::new()))],
        attachments: Vec::new(),
    }
}

fn composition(name: &str) -> CompositionId {
    CompositionId::new(name).expect("a composition id")
}

fn save(composition: &CompositionId) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition.clone(),
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Urgent".to_owned(),
        document: empty_document(),
        blobs: Vec::new(),
        then_close: false,
    })
}

fn send(composition: &CompositionId) -> Intent {
    Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Urgent".to_owned(),
        document: empty_document(),
        blobs: Vec::new(),
        composition: Some(composition.clone()),
    }
}

#[tokio::test]
async fn a_new_composer_opens_normal_and_offers_every_importance_highest_first() {
    let scratch = tempfile::TempDir::new().unwrap();
    let (app, _logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    let new = composition("new");
    app.open_composer(
        new.clone(),
        crate::ComposeOrigin::New { account: None },
        None,
        &staging_dir(&scratch, "importance-new"),
    )
    .await
    .expect("a new message opens");

    let picker = app.composer_importance(&new);
    assert_eq!(picker.selected, Importance::Normal);
    assert_eq!(
        picker.options,
        [Importance::High, Importance::Normal, Importance::Low]
    );
    assert!(!picker.changed);
}

/// What the user picks is what the server stores and what the recipient gets.
#[tokio::test(start_paused = true)]
async fn the_picked_importance_is_saved_and_sent() {
    let scratch = tempfile::TempDir::new().unwrap();
    let (app, logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    let new = composition("urgent");
    app.open_composer(
        new.clone(),
        crate::ComposeOrigin::New { account: None },
        None,
        &staging_dir(&scratch, "importance-sent"),
    )
    .await
    .expect("a new message opens");

    let picker = app.set_composer_importance(&new, Importance::High);
    assert_eq!(picker.selected, Importance::High);
    assert!(picker.changed, "a picked importance is something written");

    app.dispatch(save(&new)).await;
    dispatch_until(&app, send(&new), SendStatus::Sent)
        .await
        .await
        .expect("the send finishes");

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.last().expect("saved").0.importance, Importance::High);
    let sent = logs.submissions.lock().unwrap();
    assert_eq!(sent.last().expect("sent").importance, Importance::High);
}

/// A changed importance is a change to the draft, so the save after it writes.
#[tokio::test]
async fn changing_only_the_importance_saves_again() {
    let scratch = tempfile::TempDir::new().unwrap();
    let (app, logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    let new = composition("lowered");
    app.open_composer(
        new.clone(),
        crate::ComposeOrigin::New { account: None },
        None,
        &staging_dir(&scratch, "importance-resave"),
    )
    .await
    .expect("a new message opens");

    app.dispatch(save(&new)).await;
    app.set_composer_importance(&new, Importance::Low);
    app.dispatch(save(&new)).await;

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 2, "the second save is not the unchanged one");
    assert_eq!(puts[1].0.importance, Importance::Low);
}

/// A stored draft reopens with the importance it was saved with, and saves it again.
#[tokio::test]
async fn a_resumed_draft_keeps_its_importance() {
    let scratch = tempfile::TempDir::new().unwrap();
    let mut stored = rich_stored_draft("rich-1");
    stored.envelope.importance = Importance::High;
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored]));
    app.dispatch(Intent::RefreshMail).await;
    let resumed = composition("resumed");
    app.open_composer(
        resumed.clone(),
        crate::ComposeOrigin::Draft {
            message: MessageRef::from_parts("acct-1", "rich-1".to_owned()).unwrap(),
        },
        None,
        &staging_dir(&scratch, "importance-resumed"),
    )
    .await
    .expect("the draft resumes");

    let picker = app.composer_importance(&resumed);
    assert_eq!(picker.selected, Importance::High);
    assert!(!picker.changed, "what it was stored with is not a change");

    app.dispatch(save(&resumed)).await;
    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.last().expect("saved").0.importance, Importance::High);
}

#[tokio::test]
async fn a_composition_the_core_does_not_hold_is_normal_and_cannot_be_raised() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    let gone = composition("gone");
    assert_eq!(
        app.set_composer_importance(&gone, Importance::High)
            .selected,
        Importance::Normal
    );
    assert_eq!(app.composer_importance(&gone).selected, Importance::Normal);
}
