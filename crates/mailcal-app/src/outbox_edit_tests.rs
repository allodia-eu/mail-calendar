//! Edit on a queued send moves the message back into Drafts: the whole message, files and
//! conversation included, and never so that it is in neither place.
//!
//! Once Send is pressed the Outbox is the message's one place (`docs/sending.md`), and Edit is
//! the user moving it back. What these guard is that nothing is dropped on the way: a composer
//! that opened without a file has its first save take that file off the draft, and an edited
//! reply that lost its `In-Reply-To` arrives outside its conversation.
//!
//! A child of the send tests, reusing their `SubmitProvider` and app builders.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use engine_api::{
    AccountId, Draft, DraftAttachment, DraftCalendar, EmailAddress, MessageIdHeader, ScheduleMethod,
};
use mailcal_composer::{Block, ComposerDocument, InlineContent, Paragraph, TextRun};
use mailcal_viewmodel::QueuedRow;

use super::{SubmitProvider, app_over, outbox_tests::outbox_holding};
use crate::{App, CompositionId, DraftsIntent, Intent, OutboxIntent, QueuedRef};

/// A staging directory of this test's own.
pub(super) fn staging() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir()
        .join(format!(
            "mailcal-outbox-edit-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ))
        .to_string_lossy()
        .into_owned()
}

/// A reply carrying a file, queued because the server is not answering.
async fn queue_reply_with_a_file(app: &Arc<App<SubmitProvider>>) -> QueuedRow {
    let mut draft = Draft::new(
        MessageIdHeader::new("queued-reply@allodia.local").unwrap(),
        EmailAddress::new("me@allodia.local"),
        vec![EmailAddress::new("you@test.local")],
        "Re: Plans",
        "See the plan attached.",
    )
    .in_reply_to(
        MessageIdHeader::new("parent@remote").unwrap(),
        vec![MessageIdHeader::new("root@remote").unwrap()],
    );
    draft.attachments.push(DraftAttachment::attachment(
        "plan.pdf",
        "application/pdf",
        b"%PDF plan".to_vec(),
    ));
    app.send_draft(&AccountId::try_from("acct-1").unwrap(), &draft, None)
        .await;
    outbox_holding(app, 1).await.remove(0)
}

fn edit(row: &QueuedRow, staging: &str) -> Intent {
    Intent::Outbox(OutboxIntent::Edit {
        queued: QueuedRef::from_parts(&row.account, row.op).unwrap(),
        staging_directory: staging.to_owned(),
    })
}

fn save(composition: &str, text: &str) -> Intent {
    Intent::Drafts(DraftsIntent::Save {
        composition: CompositionId::new(composition).unwrap(),
        from: None,
        to: "you@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Re: Plans".to_owned(),
        document: ComposerDocument {
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
        },
        blobs: Vec::new(),
        then_close: false,
    })
}

/// **The regression.** Edit handed back the words alone, so a queued message lost its files
/// and its conversation on the way to the composer, and was in no folder until the composer
/// saved it. It is now a draft before it leaves the Outbox, and the composer opens on that
/// draft holding every file.
#[tokio::test(start_paused = true)]
async fn editing_a_queued_send_moves_all_of_it_back_into_drafts() {
    let provider = SubmitProvider::offline();
    let puts = provider.draft_puts();
    let app = app_over(provider);
    let row = queue_reply_with_a_file(&app).await;
    assert!(row.editable);

    let staging = staging();
    app.dispatch(edit(&row, &staging)).await;

    assert!(app.mailbox_list().outbox.is_empty(), "it left the Outbox");
    let saved = puts.lock().unwrap().clone();
    assert_eq!(saved.len(), 1, "and was saved as a draft first");
    assert_eq!(saved[0].0.attachments.len(), 1, "with its file");
    assert!(saved[0].0.in_reply_to.is_some(), "and its conversation");

    let request = app.compose_request().expect("the composer is offered");
    assert_eq!(request.subject, "Re: Plans");
    assert!(!request.composition.is_empty());
    assert_eq!(request.attachments.len(), 1);
    let file = &request.attachments[0];
    assert_eq!(file.file_name, "plan.pdf");
    assert_eq!(file.media_type, "application/pdf");
    assert!(file.path.starts_with(&staging));
    assert_eq!(std::fs::read(&file.path).unwrap(), b"%PDF plan");

    // The composer saves over that draft, and the reply stays a reply.
    app.dispatch(save(&request.composition, "See the new plan attached."))
        .await;
    let saved = puts.lock().unwrap().clone();
    assert_eq!(saved.len(), 2);
    assert!(
        saved[1].1.is_some(),
        "the composer's save replaces the draft the edit made"
    );
    assert_eq!(
        saved[1].0.in_reply_to.as_ref().map(MessageIdHeader::as_str),
        Some("parent@remote")
    );
    let _ = std::fs::remove_dir_all(staging);
}

/// A message whose files cannot be written where the composer reads them stays where it is:
/// opening a composer without them would have its first save take them off the draft.
#[tokio::test(start_paused = true)]
async fn a_message_whose_files_cannot_be_staged_stays_in_the_outbox() {
    let provider = SubmitProvider::offline();
    let puts = provider.draft_puts();
    let app = app_over(provider);
    let row = queue_reply_with_a_file(&app).await;
    // A file where the directory should be.
    let blocked = staging();
    std::fs::write(&blocked, b"not a directory").unwrap();

    app.dispatch(edit(&row, &blocked)).await;

    assert_eq!(app.mailbox_list().outbox.len(), 1);
    assert!(puts.lock().unwrap().is_empty(), "no draft was made");
    assert!(app.compose_request().is_none());
    let _ = std::fs::remove_file(blocked);
}

/// An answer to an invitation carries a calendar part no composer holds, so editing it would
/// send something else. It is not offered, and asked anyway it stays put.
#[tokio::test(start_paused = true)]
async fn an_invitation_answer_is_not_offered_for_editing() {
    let provider = SubmitProvider::offline();
    let puts = provider.draft_puts();
    let app = app_over(provider);
    let mut draft = Draft::new(
        MessageIdHeader::new("rsvp@allodia.local").unwrap(),
        EmailAddress::new("me@allodia.local"),
        vec![EmailAddress::new("organiser@test.local")],
        "Accepted: Plans",
        "Accepted.",
    );
    draft = draft.with_calendar(DraftCalendar::new(
        ScheduleMethod::Reply,
        "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n",
    ));
    app.send_draft(&AccountId::try_from("acct-1").unwrap(), &draft, None)
        .await;
    let row = outbox_holding(&app, 1).await.remove(0);
    assert!(!row.editable);

    app.dispatch(edit(&row, &staging())).await;

    assert_eq!(app.mailbox_list().outbox.len(), 1);
    assert!(puts.lock().unwrap().is_empty());
    assert!(app.compose_request().is_none());
}
