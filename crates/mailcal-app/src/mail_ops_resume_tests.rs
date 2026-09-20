//! Picking a draft back up: opening the copy on the server into a composer, saving over it
//! rather than beside it, and taking it away once the message has gone out.
//!
//! The rules are in `docs/drafts.md`. The one that decides most of these tests: a resumed
//! composer's next save **replaces** the stored copy, so everything the composer did not
//! open with is something that save removes from the user's mailbox. That turns three
//! ordinary-looking failures into data loss, and each has a test here: opening without the
//! draft's files, adopting a message that was never a draft, and adopting one whose content
//! could not be read.
//!
//! A child of [`super`] (the rich-submit tests), over [`super::mail_fake`]'s provider.

use engine_api::ProviderKey;
use mailcal_composer::{
    AttachmentDisposition, AttachmentId, Block, ComposerDocument, DraftAttachment, DraftBlobHandle,
    InlineContent, Paragraph, TextRun,
};

use super::mail_fake::{
    ThreadProvider, app_and_logs, dispatch_until, original_message, stored_draft,
};
use crate::{CompositionId, DraftsIntent, Intent, MessageRef, SendStatus};

fn composition(id: &str) -> CompositionId {
    CompositionId::new(id).expect("a composition id")
}

fn draft_ref(key: &str) -> MessageRef {
    MessageRef::from_parts("acct-1", key.to_owned()).expect("a message reference")
}

/// A one-paragraph document carrying `text`; enough to render, which is all these assert on.
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

/// What a resumed composer dispatches when its idle timer fires.
fn save(id: &str, text: &str) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: composition(id),
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document(text),
        blobs: Vec::new(),
    })
}

/// A directory of this test's own, emptied first so a previous run cannot make an assertion
/// pass.
fn staging_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mailcal-resume-staging-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

/// A resumed draft opens as the message it is: its recipients, its subject and its words.
///
/// The `Bcc` is empty because the stored copy carries none, which is what most transports
/// hand back, and the answer says so rather than inventing one.
#[tokio::test]
async fn a_resumed_draft_opens_with_what_the_stored_copy_holds() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;

    let resumed = app
        .resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("opens"))
        .await
        .expect("the draft resumes");

    assert_eq!(resumed.account, "acct-1");
    assert_eq!(resumed.to, "you@remote.test");
    assert_eq!(resumed.cc, "cc@remote.test");
    assert_eq!(resumed.bcc, "");
    assert_eq!(resumed.subject, "Half a subject");
    assert_eq!(resumed.body_text.trim(), "Half a sentence");
}

/// **The contract.** The composer a resume opens saves **over** the copy it came from.
///
/// Without the adoption the save has nothing to replace, the provider stores a second copy,
/// and the user's Drafts folder collects one more of the same half-written message every
/// time they put it down. Asserted on `replacing` rather than on a count, because a fake
/// that echoed the key back would let a caller that never names what it supersedes pass.
#[tokio::test]
async fn a_save_from_a_resumed_composer_supersedes_the_stored_draft() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(
        composition("c1"),
        draft_ref("d1"),
        &staging_dir("supersede"),
    )
    .await
    .expect("the draft resumes");

    app.dispatch(save("c1", "A whole sentence")).await;

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 1);
    assert_eq!(
        puts[0].1,
        Some(ProviderKey::new("d1").unwrap()),
        "the first save after a resume names the copy it came from"
    );
}

/// Every save of a resumed composition carries the **draft's own** `Message-ID`, not a fresh
/// one.
///
/// That header is what the engine keys a queued save's resource on, so a resumed composer
/// that minted its own would queue its saves beside the ones the composer made before it was
/// closed, rather than superseding them.
#[tokio::test]
async fn a_resumed_composition_keeps_the_drafts_own_message_id() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(
        composition("c1"),
        draft_ref("d1"),
        &staging_dir("message-id"),
    )
    .await
    .expect("the draft resumes");

    app.dispatch(save("c1", "One")).await;
    app.dispatch(save("c1", "Two")).await;

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 2);
    for (draft, _) in puts.iter() {
        assert_eq!(draft.message_id.as_str(), "half@allodia.local");
    }
}

/// The composer opens holding the draft's files, so a save from it keeps them.
///
/// A resumed composer that opened empty-handed would not merely *show* less: its next save
/// replaces the stored copy, and the file the user attached yesterday would be gone from
/// their mailbox with nothing said.
#[tokio::test]
async fn a_resumed_draft_opens_holding_its_files() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;

    let resumed = app
        .resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("files"))
        .await
        .expect("the draft resumes");

    assert_eq!(resumed.attachments.len(), 1);
    let file = &resumed.attachments[0];
    assert_eq!(file.file_name, "terms.pdf");
    assert_eq!(file.media_type, "application/pdf");
    assert_eq!(std::fs::read(&file.path).unwrap(), b"TERMS");
}

/// A draft whose **content** cannot be read is not resumed, and nothing is adopted.
///
/// One unreachable source is both halves of the answer here: the words and the files come
/// out of the same bytes. What matters is that the composition is left unknown, which the
/// following save proves by having nothing to replace: a composer that opened on an error
/// and then saved anyway would replace the draft with whatever was on screen.
#[tokio::test]
async fn a_draft_whose_content_cannot_be_read_is_not_resumed() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).failing_source());
    app.dispatch(Intent::RefreshMail).await;

    let refused = app
        .resume_draft(
            composition("c1"),
            draft_ref("d1"),
            &staging_dir("unreadable"),
        )
        .await;
    assert!(refused.is_err(), "a draft that cannot be read is refused");

    app.dispatch(save("c1", "Something else")).await;
    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 1);
    assert_eq!(
        puts[0].1, None,
        "a refused resume adopts nothing, so the save that follows replaces nothing"
    );
}

/// A draft whose files cannot be **written** is not resumed either, and still adopts
/// nothing.
///
/// The read succeeds here and the staging is what fails, which is the order the adoption has
/// to survive: the body arrived, so a resume that recorded the key as soon as it had the
/// words would hand the composer a draft it could save over without its file. Driven with a
/// directory that cannot exist (its parent is a file), which is what a full disk or a
/// refused path looks like from here.
#[tokio::test]
async fn a_draft_whose_files_cannot_be_staged_is_not_resumed() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    let blocked = std::env::temp_dir().join("mailcal-resume-staging-blocked");
    let _ = std::fs::remove_dir_all(&blocked);
    std::fs::write(&blocked, b"not a directory").unwrap();
    let directory = blocked.join("inside").to_string_lossy().into_owned();

    let refused = app
        .resume_draft(composition("c1"), draft_ref("d1"), &directory)
        .await;
    assert!(
        refused.is_err(),
        "a draft whose files cannot be written is refused"
    );

    app.dispatch(save("c1", "Something else")).await;
    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(
        puts[0].1, None,
        "a refused resume adopts nothing, so the save that follows replaces nothing"
    );
}

/// A message that is not a draft is refused, and nothing is adopted.
///
/// The guard is what keeps a resume from being a way to edit received mail: the composition
/// would take the message's key, and its first save would rewrite the sender's message while
/// a discard would delete it. Both halves are asserted, because refusing the call while
/// still recording the key would leave the second one live.
#[tokio::test]
async fn a_message_that_is_not_a_draft_is_never_resumed() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![original_message("m1")]));
    app.dispatch(Intent::RefreshMail).await;

    let refused = app
        .resume_draft(
            composition("c1"),
            draft_ref("m1"),
            &staging_dir("not-a-draft"),
        )
        .await;
    assert!(refused.is_err(), "ordinary mail is not a draft");

    app.dispatch(save("c1", "Typing into it")).await;
    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts[0].1, None, "no received message's key is adopted");
    assert!(
        logs.draft_deletes.lock().unwrap().is_empty(),
        "and nothing about the received message is removed"
    );
}

/// Discarding a resumed composition removes the copy on the server.
///
/// The composition never saved anything itself, so this only works because the resume gave
/// it the stored draft's key; without that a discard from a resumed composer would quietly
/// do nothing and leave the draft the user threw away in their mailbox.
#[tokio::test]
async fn discarding_a_resumed_draft_removes_the_stored_copy() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("discard"))
        .await
        .expect("the draft resumes");

    app.dispatch(Intent::Drafts(DraftsIntent::Discard {
        composition: composition("c1"),
    }))
    .await;

    let deletes = logs.draft_deletes.lock().unwrap();
    assert_eq!(deletes.as_slice(), &[ProviderKey::new("d1").unwrap()]);
}

/// A send that was accepted takes the composition's stored draft away.
///
/// Otherwise every message composed over more than half a minute is filed twice: once in
/// Sent, where the user expects it, and once in Drafts, where they find it weeks later and
/// cannot tell whether it went.
#[tokio::test(start_paused = true)]
async fn an_accepted_send_takes_the_stored_draft_away() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("sent"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("Finished at last"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    let deletes = logs.draft_deletes.lock().unwrap();
    assert_eq!(
        deletes.as_slice(),
        &[ProviderKey::new("d1").unwrap()],
        "the message has gone out, so the copy waiting in Drafts goes with it"
    );
}

/// A send that **failed** keeps the draft.
///
/// This is the one outcome where the stored copy is the only one left: the message never
/// reached a server, nothing will retry it, and the host has already dismissed the composer
/// that held the words. Removing it here would be the send losing the user's message.
#[tokio::test(start_paused = true)]
async fn a_failed_send_leaves_the_draft_where_the_words_are() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).failing_send());
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("failed"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("Never got there"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Failed).await;

    assert!(
        logs.draft_deletes.lock().unwrap().is_empty(),
        "nothing else holds the message, so the draft stays"
    );
}

/// A send from a composer that never saved reaches no server about a draft.
///
/// The ordinary case: most messages are written and sent inside the idle interval, so the
/// cleanup must be a no-op rather than a delete of whatever key happens to be around.
#[tokio::test(start_paused = true)]
async fn a_send_from_a_composer_that_never_saved_removes_nothing() {
    let (app, logs) = app_and_logs(ThreadProvider::with(Vec::new()));
    app.dispatch(Intent::RefreshMail).await;

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Quick one".to_owned(),
        document: document("Sent straight away"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    assert!(logs.draft_deletes.lock().unwrap().is_empty());
}

/// A send with no network takes the draft away too: the message is **in the outbox**, which
/// is durable, so the stored copy is already a duplicate of something on its way.
///
/// The rule is "accepted", not "delivered". Waiting for delivery instead would leave a
/// message sent from a train filed in Drafts for good, because nothing revisits the
/// composition once the drain settles it.
#[tokio::test(start_paused = true)]
async fn a_queued_send_takes_the_stored_draft_away_as_well() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]).queueing_send());
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(composition("c1"), draft_ref("d1"), &staging_dir("queued"))
        .await
        .expect("the draft resumes");

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: document("On the train"),
        blobs: Vec::new(),
        composition: Some(composition("c1")),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Queued).await;

    let deletes = logs.draft_deletes.lock().unwrap();
    assert_eq!(deletes.as_slice(), &[ProviderKey::new("d1").unwrap()]);
}

/// A save carries the composer's files, so a resumed draft keeps the one it opened with.
///
/// The save is the write that *replaces* the stored copy, so this is not about showing less:
/// a save rendered without the attachment takes the user's file off the server while they are
/// still writing the message it belongs to. Asserted on the stored draft rather than on the
/// document, because what reaches the provider is the only thing that survives the round trip.
#[tokio::test]
async fn a_save_keeps_the_file_the_composer_is_holding() {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![stored_draft("d1")]));
    app.dispatch(Intent::RefreshMail).await;
    app.resume_draft(
        composition("c1"),
        draft_ref("d1"),
        &staging_dir("keeps-files"),
    )
    .await
    .expect("the draft resumes");

    let handle = DraftBlobHandle::new("resumed-file").unwrap();
    let mut document = document("Still writing");
    document.attachments.push(DraftAttachment {
        id: AttachmentId::new("resumed-file").unwrap(),
        blob: Some(handle.clone()),
        file_name: "terms.pdf".to_owned(),
        media_type: "application/pdf".to_owned(),
        size: Some(5),
        disposition: AttachmentDisposition::Attachment,
        data_url: None,
    });
    app.dispatch(Intent::Drafts(DraftsIntent::Save {
        composition: composition("c1"),
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document,
        blobs: vec![crate::ComposerBlob::new(handle, b"TERMS".to_vec())],
    }))
    .await;

    let puts = logs.draft_puts.lock().unwrap();
    assert_eq!(puts.len(), 1);
    let stored = &puts[0].0;
    assert_eq!(stored.attachments.len(), 1);
    assert_eq!(stored.attachments[0].file_name, "terms.pdf");
    assert_eq!(stored.attachments[0].content, b"TERMS");
}
