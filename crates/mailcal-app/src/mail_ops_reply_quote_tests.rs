//! Tests for **submit-time quote-body hardening**: the shared rich-reply path re-sanitises a
//! quoted original before sending (dropping script the WebView editor could have round-tripped),
//! and turns the reading view's resolved inline `data:` images back into `cid:` references to
//! re-attached parts, preserving each part's original `Content-ID`. A child of [`super`] (the
//! reply/forward tests), reusing its `reply_app`/`original_message`/`dispatch_until` fixtures;
//! split into its own file so each test module stays under the 500-line limit.

use engine_api::ContentIdHeader;
use mailcal_composer::{Block, ComposerDocument, Paragraph, Quote, QuoteAttribution, QuoteStyle};

use super::{dispatch_until, original_message, reply_app};
use crate::{Intent, MessageRef, SendStatus};

#[tokio::test(start_paused = true)]
async fn rich_reply_re_sanitizes_a_quoted_original_before_sending() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    // A quote whose body carries a hostile <script> the WebView editor could have round-tripped,
    // plus benign formatted content. The sent draft must drop the script and keep the safe markup
    //: the shared-core sanitisation gate, exercised end-to-end through a real reply submit.
    let document = ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: Vec::new(),
            }),
            Block::Quote(Quote {
                style: QuoteStyle::Indented,
                attribution: QuoteAttribution {
                    line: "On 30 Jun 2026, sender@remote.test wrote:".to_owned(),
                    headers: Vec::new(),
                },
                body_html: "<p>Hi <strong>there</strong></p><script>alert(1)</script>".to_owned(),
                body_plain: "Hi there".to_owned(),
            }),
        ],
        attachments: Vec::new(),
    };
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs: Vec::new(),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    let draft = &submissions[0];
    let html = draft
        .html_body
        .as_deref()
        .expect("rich reply has an HTML body");
    // The script and its content are gone; the benign quoted markup survives in the blockquote.
    assert!(
        !html.contains("<script") && !html.contains("alert(1)"),
        "the quoted body's script must be stripped before sending: {html}"
    );
    assert!(html.contains("Hi <strong>there</strong>"));
    assert!(html.contains("<blockquote"));
    assert!(html.contains("On 30 Jun 2026, sender@remote.test wrote:"));
}

#[tokio::test(start_paused = true)]
async fn rich_reply_reattaches_quoted_inline_images_as_cid_keeping_the_original_id() {
    let (app, submissions) = reply_app(vec![original_message("m1")]);
    app.dispatch(Intent::RefreshMail).await;

    // The reading view resolved the original's inline `cid:` image to a self-contained `data:`
    // URI, so the reply quote seeds from that `data:` markup. On send, the quoted `data:` image
    // must be turned back into a `cid:` reference to the original part; with the SAME
    // Content-ID it had inbound, and that part re-attached, not left as a `data:` URI (which
    // Outlook's reader blocks). `aGVsbG8=` is base64 for the part's bytes (`hello`).
    let document = ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: Vec::new(),
            }),
            Block::Quote(Quote {
                style: QuoteStyle::Indented,
                attribution: QuoteAttribution {
                    line: "On 30 Jun 2026, sender@remote.test wrote:".to_owned(),
                    headers: Vec::new(),
                },
                body_html: "<p>Logo:</p><img src=\"data:image/png;base64,aGVsbG8=\">".to_owned(),
                body_plain: "Logo:".to_owned(),
            }),
        ],
        attachments: Vec::new(),
    };
    let intent = Intent::SubmitRichReply {
        message: MessageRef::from_parts("acct-1", "m1".to_owned()).unwrap(),
        from: None,
        to: "reply@remote.test".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: None,
        document,
        blobs: Vec::new(),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;
    assert_eq!(app.send_status(), SendStatus::Sent);

    let submissions = submissions.lock().unwrap();
    let draft = &submissions[0];
    let html = draft
        .html_body
        .as_deref()
        .expect("rich reply has an HTML body");
    // The quoted `data:` image is now a `cid:` reference to the original id, and no `data:`
    // image survives in the sent body.
    assert!(
        html.contains("src=\"cid:part1.demo@allodia.local\""),
        "quoted inline image must re-reference its original cid: {html}"
    );
    assert!(
        !html.contains("data:image"),
        "no data: image may remain in the sent body: {html}"
    );

    // The part is re-attached inline, addressed by its original Content-ID, with its original
    // media type and bytes preserved.
    let inline: Vec<_> = draft.attachments.iter().filter(|a| a.is_inline()).collect();
    assert_eq!(inline.len(), 1, "exactly one inline part re-attached");
    let part = inline[0];
    assert_eq!(
        part.content_id().map(ContentIdHeader::as_str),
        Some("part1.demo@allodia.local")
    );
    assert_eq!(part.media_type, "image/png");
    assert_eq!(part.content, b"hello");
}
