//! A message reopened in a composer opens whole, and keeps on every save and on the send what
//! the editor cannot hold: the conversation it answers, and the parts its quoted pictures came
//! from (`docs/drafts.md`).
//!
//! A child of [`super`] (the rich-submit tests), over [`super::mail_fake`]'s provider and its
//! formatted reply ([`super::mail_fake::rich_stored_draft`]).

use engine_api::{Draft, DraftAttachmentDisposition, MessageIdHeader};
use mailcal_composer::{
    Block, ComposerDocument, InlineContent, Paragraph, Quote, QuoteAttribution, QuoteStyle, TextRun,
};

use super::mail_fake::{ThreadProvider, app_and_logs, dispatch_until, rich_stored_draft};
use crate::{CompositionId, DraftsIntent, Intent, MessageRef, SendStatus};

/// The picture the stored reply's quote shows, as the editor holds it once the core has
/// turned its part into a `data:` URI.
const QUOTED_PICTURE: &str = "<img src=\"data:image/png;base64,aGVsbG8=\" alt=\"chart\">";

fn composition() -> CompositionId {
    CompositionId::new("c1").expect("a composition id")
}

fn draft_ref() -> MessageRef {
    MessageRef::from_parts("acct-1", "rich-1".to_owned()).expect("a message reference")
}

fn staging_dir(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mailcal-reopen-staging-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

/// What the editor hands back for the reopened reply: the user's line over the quote, whose
/// body still shows the picture as the `data:` URI it opened with.
fn reply_document() -> ComposerDocument {
    ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: vec![InlineContent::Text(TextRun {
                    text: "A whole sentence".to_owned(),
                    bold: true,
                    italic: false,
                    underline: false,
                    font_size: None,
                    color: None,
                    highlight: None,
                    link: None,
                })],
            }),
            Block::Quote(Quote {
                style: QuoteStyle::Indented,
                attribution: QuoteAttribution {
                    line: "On Monday, Bob wrote:".to_owned(),
                    headers: Vec::new(),
                },
                body_html: QUOTED_PICTURE.to_owned(),
                body_plain: String::new(),
            }),
        ],
        attachments: Vec::new(),
    }
}

async fn resumed() -> (
    std::sync::Arc<crate::App<ThreadProvider>>,
    super::mail_fake::ProviderLogs,
) {
    let (app, logs) = app_and_logs(ThreadProvider::with(vec![rich_stored_draft("rich-1")]));
    app.dispatch(Intent::RefreshMail).await;
    let request = app
        .resume_draft(composition(), draft_ref(), &staging_dir("resumed"))
        .await
        .expect("the draft resumes");
    assert!(
        !request.body_html.is_empty(),
        "the formatted body is handed over"
    );
    (app, logs)
}

fn assert_quoted_picture_is_its_part(draft: &Draft) {
    let html = draft.html_body.as_deref().unwrap_or_default();
    assert!(
        html.contains("cid:chart@remote.test") && !html.contains("data:image/png"),
        "the quoted picture points at its part again"
    );
    assert!(
        draft.attachments.iter().any(|part| matches!(
            &part.disposition,
            DraftAttachmentDisposition::Inline { content_id }
                if content_id.as_str() == "chart@remote.test"
        ) && part.content == b"hello"),
        "the part the picture came from goes with it"
    );
}

fn assert_in_conversation(draft: &Draft) {
    assert_eq!(
        draft.in_reply_to,
        Some(MessageIdHeader::new("parent@remote").unwrap()),
        "the reply still answers its message"
    );
    assert_eq!(
        draft.references,
        vec![
            MessageIdHeader::new("root@remote").unwrap(),
            MessageIdHeader::new("parent@remote").unwrap(),
        ]
    );
}

/// The composer gets the body the user wrote, made safe, with its picture showable: the
/// formatting, the quote and the picture all reach the editor, and nothing that could run.
#[tokio::test]
async fn a_resumed_draft_opens_formatted_with_its_pictures_inline() {
    let (app, _logs) = app_and_logs(ThreadProvider::with(vec![rich_stored_draft("rich-1")]));
    app.dispatch(Intent::RefreshMail).await;

    let request = app
        .resume_draft(composition(), draft_ref(), &staging_dir("formatted"))
        .await
        .expect("the draft resumes");

    let html = &request.body_html;
    assert!(html.contains("<strong>Half</strong> a sentence"), "{html}");
    assert!(
        html.contains("<blockquote"),
        "the quote is handed over: {html}"
    );
    assert!(
        html.contains("src=\"data:image/png;base64,aGVsbG8=\""),
        "the quoted picture comes as bytes the editor can show: {html}"
    );
    assert!(
        !html.contains("steal"),
        "nothing that could run reaches the editor: {html}"
    );
    assert!(
        !html.contains("cid:"),
        "no picture is left pointing at a part: {html}"
    );
    assert_eq!(request.attachments.len(), 1, "the file is staged as before");
    assert!(
        request.body_text.contains("a sentence"),
        "{:?}",
        request.body_text
    );
}

/// A resumed reply's saves stay in the conversation it answers. The composer cannot hold its
/// threading headers, so the composition does; without it the next save quietly turned the
/// reply into a new message.
#[tokio::test]
async fn a_save_of_a_resumed_reply_keeps_its_conversation_and_its_quoted_picture() {
    let (app, logs) = resumed().await;

    app.dispatch(Intent::Drafts(DraftsIntent::Save {
        composition: composition(),
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: reply_document(),
        blobs: Vec::new(),
        then_close: false,
    }))
    .await;

    let puts = logs.draft_puts.lock().unwrap();
    let (saved, _) = puts.last().expect("the draft is saved");
    assert_in_conversation(saved);
    assert_quoted_picture_is_its_part(saved);
}

/// The send carries the same two things the saves do.
#[tokio::test(start_paused = true)]
async fn the_send_of_a_resumed_reply_keeps_its_conversation_and_its_quoted_picture() {
    let (app, logs) = resumed().await;

    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Half a subject".to_owned(),
        document: reply_document(),
        blobs: Vec::new(),
        composition: Some(composition()),
    };
    dispatch_until(&app, intent, SendStatus::Sent)
        .await
        .await
        .expect("the send finishes");

    let submissions = logs.submissions.lock().unwrap();
    let sent = submissions.last().expect("the message is sent");
    assert_in_conversation(sent);
    assert_quoted_picture_is_its_part(sent);
}
