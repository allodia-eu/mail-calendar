//! A drafted reply's stored draft (`docs/ai.md` rule 25): every save of a drafted composition asks
//! for the drafted-reply keyword, a mark the server did not keep is noted on this device, and a
//! stored draft carrying the mark resumes as a drafted reply, without the card, which is not
//! stored.
//!
//! A child of the drafted-reply send tests, for their `sent` and the rich-submit tests' fakes.
//! Every address is `@test.local` or a fixture's `.test`; nothing is sent anywhere.

use std::sync::Arc;

use engine_api::{MessageIdHeader, MessageMetadata};
use mailcal_account::drafted_reply_keyword;
use mailcal_composer::{Block, ComposerDocument, InlineContent, LeadText, Paragraph, TextRun};

use super::{
    super::{
        mail_fake::{
            ProviderLogs, ThreadProvider, original_message, rich_stored_draft, staging_dir,
        },
        reply_draft_support::{
            ScriptedRelay, assistant, drafted, drafted_into, open_reply, report, view,
        },
    },
    sent,
};
use crate::{
    App, ComposeOrigin, CompositionId, DraftsIntent, Intent, MessageRef, ai::ReplyDraftState,
};

fn save(composition: &CompositionId, text: &str) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition.clone(),
        from: None,
        to: "reply@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Re: Quarterly report".to_owned(),
        document: ComposerDocument {
            blocks: vec![Block::Paragraph(Paragraph::new(vec![InlineContent::Text(
                TextRun {
                    font: None,
                    text: text.to_owned(),
                    bold: false,
                    italic: false,
                    underline: false,
                    font_size: None,
                    color: None,
                    highlight: None,
                    link: None,
                },
            )]))],
            attachments: Vec::new(),
        },
        blobs: Vec::new(),
        then_close: false,
    })
}

fn marked() -> MessageMetadata {
    MessageMetadata::default().with_keyword(drafted_reply_keyword())
}

/// The metadata the last save asked for, and the `Message-ID` it stored.
fn last_save(logs: &ProviderLogs) -> (MessageMetadata, MessageIdHeader) {
    let metadata = logs.draft_metadata.lock().unwrap().last().cloned().unwrap();
    let puts = logs.draft_puts.lock().unwrap();
    (metadata, puts.last().unwrap().0.message_id.clone())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_drafted_reply_is_stored_with_the_keyword_on_every_save() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, logs) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;

    app.dispatch(save(&reply, "My own start.")).await;
    assert_eq!(
        last_save(&logs).0,
        MessageMetadata::default(),
        "nothing drafted yet"
    );

    let serial = drafted_into(&app, &reply).await;
    app.dispatch(save(&reply, "Friday works.")).await;
    assert_eq!(last_save(&logs).0, marked());

    // Undone and rewritten: what is stored may still carry its words, so the mark stays.
    app.take_reply_draft_report(
        &reply,
        report(serial, LeadText::Written, false, vec![], 2),
        0,
    );
    app.dispatch(save(&reply, "Friday works, rewritten.")).await;
    assert_eq!(last_save(&logs).0, marked());
    assert_eq!(logs.draft_puts.lock().unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mark_the_server_did_not_keep_is_noted_on_this_device() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let provider = ThreadProvider::with(vec![original_message("m1")]).keeping_no_draft_keywords();
    let (app, logs) = assistant(provider, &relay).await;
    let reply = open_reply(&app, "reply").await;
    drafted_into(&app, &reply).await;

    app.dispatch(save(&reply, "Friday works.")).await;
    let (asked, message_id) = last_save(&logs);
    assert_eq!(asked, marked(), "asked for all the same");
    assert!(app.is_drafted_reply(&message_id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mark_the_server_kept_needs_no_note_on_this_device() {
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, logs) = assistant(ThreadProvider::with(vec![original_message("m1")]), &relay).await;
    let reply = open_reply(&app, "reply").await;
    drafted_into(&app, &reply).await;

    app.dispatch(save(&reply, "Friday works.")).await;
    let (_, message_id) = last_save(&logs);
    assert!(
        !app.is_drafted_reply(&message_id),
        "the keyword travels with it"
    );
}

/// Resumes the stored draft `key` into a composer named `name`.
async fn resume(app: &App<ThreadProvider>, key: &str, name: &str) -> CompositionId {
    let scratch = tempfile::tempdir().unwrap();
    let composition = CompositionId::new(name).unwrap();
    app.open_composer(
        composition.clone(),
        ComposeOrigin::Draft {
            message: MessageRef::from_parts("acct-1", key.to_owned()).unwrap(),
        },
        None,
        &staging_dir(&scratch, name),
    )
    .await
    .expect("the draft resumes");
    composition
}

/// On this device after a restart or on another one: the mark is read from the synced message.
/// The card and its checklist are not stored, so the reply resumes without them and Send asks
/// nothing; its saves and its send carry the mark.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stored_draft_carrying_the_mark_resumes_as_a_drafted_reply() {
    let mut stored = rich_stored_draft("rich-drafted");
    stored.keywords.insert(drafted_reply_keyword());
    let provider = ThreadProvider::with(vec![original_message("m1"), stored]);
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, logs) = assistant(provider, &relay).await;

    let resumed = resume(&app, "rich-drafted", "resumed").await;
    assert!(app.composition_drafted(&resumed));
    let card = view(&app, &resumed);
    assert_eq!(card.state, ReplyDraftState::Ready, "no card to draw");
    assert_eq!(card.send_question, None);
    assert!(card.editor_script.is_none());

    app.dispatch(save(&resumed, "Friday works.")).await;
    assert_eq!(last_save(&logs).0, marked());
    let gone = sent(&Arc::clone(&app), &logs, &resumed).await;
    assert!(
        gone.sent_copy_metadata
            .keywords
            .contains(&drafted_reply_keyword())
    );
    assert!(app.is_drafted_reply(&gone.message_id));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stored_draft_is_drafted_by_the_mark_or_by_this_devices_log() {
    let provider = ThreadProvider::with(vec![
        original_message("m1"),
        rich_stored_draft("rich-plain"),
        rich_stored_draft("rich-logged"),
    ]);
    let relay = ScriptedRelay::answering(Ok(drafted()));
    let (app, logs) = assistant(provider, &relay).await;
    // Both fixtures carry the same `Message-ID`; the log names it only for the second resume.
    let plain = resume(&app, "rich-plain", "plain").await;
    assert!(!app.composition_drafted(&plain));
    app.dispatch(save(&plain, "All mine.")).await;
    assert_eq!(last_save(&logs).0, MessageMetadata::default());

    app.note_drafted_reply(
        "acct-1",
        MessageIdHeader::new("half@allodia.local").unwrap(),
    );
    let logged = resume(&app, "rich-logged", "logged").await;
    assert!(app.composition_drafted(&logged));
}

/// A save queued for later is answered by no receipt; its keywords are read back by a sync. Until
/// then this device notes the mark it asked for.
#[tokio::test]
async fn a_queued_save_of_a_drafted_reply_is_noted_on_this_device() {
    let (app, _) = super::super::mail_fake::app_and_logs(ThreadProvider::with(Vec::new()));
    let account = engine_api::AccountId::try_from("acct-1").unwrap();
    let queued = MessageIdHeader::new("queued@allodia.local").unwrap();
    app.drafted_save_settled(&account, &queued, &marked(), None);
    assert!(app.is_drafted_reply(&queued));
    let plain = MessageIdHeader::new("plain@allodia.local").unwrap();
    app.drafted_save_settled(&account, &plain, &MessageMetadata::default(), None);
    assert!(
        !app.is_drafted_reply(&plain),
        "nothing asked, nothing noted"
    );
}
