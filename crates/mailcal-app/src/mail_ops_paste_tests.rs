//! What a picture the editor captured itself turns into on the wire.
//!
//! A child of [`super`] (the submit tests), reusing its `submit_app`/`dispatch_until` fixtures;
//! split into its own file so each test module stays under the 500-line limit.

use engine_api::DraftAttachmentDisposition;
use mailcal_composer::{
    AttachmentDisposition, AttachmentId, Block, ComposerDocument,
    DraftAttachment as ComposerAttachment, InlineContent, InlineImage, Paragraph,
};

use super::{dispatch_until, submit_app};
use crate::{Intent, SendStatus};

/// A document holding one picture the editor captured itself: the shape `composerDocument()`
/// emits after a paste, with the bytes in the attachment rather than behind a host handle.
fn pasted_image_document() -> ComposerDocument {
    // A 1×1 transparent PNG: small enough to inline here, real enough to decode.
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let id = AttachmentId::new("pasted-0").unwrap();
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![InlineContent::Image(InlineImage {
                attachment_id: id.clone(),
                alt_text: String::new(),
                width_px: None,
            })],
        })],
        attachments: vec![ComposerAttachment {
            id,
            blob: None,
            file_name: "pasted-image.png".to_owned(),
            media_type: "image/png".to_owned(),
            size: None,
            disposition: AttachmentDisposition::Inline {
                cid: mailcal_composer::ContentId::new("pasted@test.local").unwrap(),
            },
            data_url: Some(PNG.to_owned()),
        }],
    }
}

#[tokio::test(start_paused = true)]
async fn a_pasted_picture_leaves_as_a_cid_part_with_no_host_blob() {
    // The paste path has no staged file to hand a blob handle for, so its bytes travel in the
    // document as a `data:` URI. What must come out is what a host-picked inline image produces:
    // a `multipart/related` part the body references by `cid:`, never a `data:` image (an Outlook
    // reader blocks those).
    let (app, submissions) = submit_app();
    let intent = Intent::SubmitRichMail {
        from: None,
        to: "you@test.local".to_owned(),
        cc: String::new(),
        bcc: String::new(),
        subject: "Screenshot".to_owned(),
        document: pasted_image_document(),
        blobs: Vec::new(),
    };
    let _task = dispatch_until(&app, intent, SendStatus::Sent).await;

    let submissions = submissions.lock().unwrap();
    let draft = &submissions[0];
    let html = draft.html_body.as_deref().expect("an HTML body");
    assert!(
        html.contains("<img src=\"cid:pasted@test.local\""),
        "{html}"
    );
    assert!(!html.contains("data:image"), "{html}");
    assert_eq!(draft.attachments.len(), 1);
    assert_eq!(draft.attachments[0].media_type, "image/png");
    assert!(!draft.attachments[0].content.is_empty());
    match &draft.attachments[0].disposition {
        DraftAttachmentDisposition::Inline { content_id } => {
            assert_eq!(content_id.as_str(), "pasted@test.local");
        }
        DraftAttachmentDisposition::Attachment => panic!("a pasted picture is an inline part"),
    }
}
