use super::*;

#[test]
fn renders_inline_images_as_cid_parts_and_keeps_regular_attachments_separate() {
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![
                text("See "),
                InlineContent::Image(InlineImage {
                    attachment_id: aid("img-1"),
                    alt_text: "quarterly chart".to_owned(),
                    width_px: Some(320),
                }),
            ],
        })],
        attachments: vec![inline_attachment("img-1"), regular_attachment("file-1")],
    };

    let output = render(&document).expect("valid document");

    assert_eq!(
        output.html,
        html_doc(
            "<p>See <img src=\"cid:chart-1@example.test\" alt=\"quarterly chart\" width=\"320\"></p>"
        )
    );
    assert_eq!(output.plain_text, "See [quarterly chart]");
    assert_eq!(output.inline_attachments.len(), 1);
    assert_eq!(
        output.inline_attachments[0].cid.as_ref(),
        Some(&cid("chart-1@example.test"))
    );
    assert_eq!(output.attachments.len(), 1);
    assert_eq!(output.attachments[0].file_name, "report.pdf");
}

#[test]
fn inline_image_sharing_a_blob_with_an_attachment_still_emits_its_cid_part() {
    // The same blob is used both inline (a CID part the body references) and as a regular
    // attachment. Both must be emitted: deduping inline by blob (shared with the attachment
    // pool) would drop the inline part and leave the <img src="cid:..."> dangling.
    let inline = DraftAttachment {
        id: aid("img-1"),
        blob: Some(blob("blob://shared")),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Inline {
            cid: cid("chart-1@example.test"),
        },
        data_url: None,
    };
    let attached = DraftAttachment {
        id: aid("file-1"),
        blob: Some(blob("blob://shared")),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Attachment,
        data_url: None,
    };
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![InlineContent::Image(InlineImage {
                attachment_id: aid("img-1"),
                alt_text: "chart".to_owned(),
                width_px: None,
            })],
        })],
        attachments: vec![inline, attached],
    };

    let output = render(&document).expect("valid document");
    assert_eq!(
        output.inline_attachments.len(),
        1,
        "the inline CID part the body references is emitted"
    );
    assert_eq!(
        output.inline_attachments[0].cid.as_ref(),
        Some(&cid("chart-1@example.test"))
    );
    assert_eq!(
        output.attachments.len(),
        1,
        "the regular attachment on the shared blob is emitted too"
    );
    assert!(
        output
            .html
            .contains("<img src=\"cid:chart-1@example.test\"")
    );
}

#[test]
fn two_inline_images_sharing_a_blob_under_distinct_cids_emit_both_parts() {
    // Two inline images reuse one blob (same bytes) but carry distinct CIDs. Each CID the
    // body references must get its own part; deduping by blob would drop the second.
    let make_inline = |id: &str, content_id: &str| DraftAttachment {
        id: aid(id),
        blob: Some(blob("blob://shared")),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Inline {
            cid: cid(content_id),
        },
        data_url: None,
    };
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![
                InlineContent::Image(InlineImage {
                    attachment_id: aid("img-1"),
                    alt_text: "a".to_owned(),
                    width_px: None,
                }),
                InlineContent::Image(InlineImage {
                    attachment_id: aid("img-2"),
                    alt_text: "b".to_owned(),
                    width_px: None,
                }),
            ],
        })],
        attachments: vec![
            make_inline("img-1", "c1@example.test"),
            make_inline("img-2", "c2@example.test"),
        ],
    };

    let output = render(&document).expect("valid document");
    assert_eq!(output.inline_attachments.len(), 2);
    let cids: Vec<&str> = output
        .inline_attachments
        .iter()
        .filter_map(|a| a.cid.as_ref().map(crate::ContentId::as_str))
        .collect();
    assert!(cids.contains(&"c1@example.test") && cids.contains(&"c2@example.test"));
}

#[test]
fn rejects_inline_image_without_matching_cid_attachment() {
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![InlineContent::Image(InlineImage {
                attachment_id: aid("missing"),
                alt_text: String::new(),
                width_px: None,
            })],
        })],
        attachments: Vec::new(),
    };

    assert!(matches!(
        render(&document),
        Err(ComposerError::MissingInlineAttachment { .. })
    ));
}

#[test]
fn rejects_unreferenced_inline_attachment_so_it_never_reaches_the_manifest() {
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![text("no image here")],
        })],
        attachments: vec![inline_attachment("img-1")],
    };

    assert!(matches!(
        render(&document),
        Err(ComposerError::UnusedInlineAttachment { .. })
    ));
}

/// A one-pixel PNG as the editor would hand it over after a paste.
const PASTED_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

fn pasted_document(attachment: DraftAttachment) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![InlineContent::Image(InlineImage {
                attachment_id: aid("img-1"),
                alt_text: "screenshot".to_owned(),
                width_px: None,
            })],
        })],
        attachments: vec![attachment],
    }
}

#[test]
fn a_pasted_image_renders_as_a_cid_part_with_no_host_blob() {
    // The paste path carries its own bytes: there is no staged file to hand a handle for, so the
    // manifest names the part by CID and leaves the blob empty for the core to fill from the
    // `data:` URI.
    let output = render(&pasted_document(pasted_attachment("img-1", PASTED_PNG)))
        .expect("a pasted image is a valid document");

    assert_eq!(output.inline_attachments.len(), 1);
    assert!(output.inline_attachments[0].blob.is_none());
    assert_eq!(
        output.inline_attachments[0].cid.as_ref(),
        Some(&cid("pasted-1@example.test"))
    );
    assert!(
        output
            .html
            .contains("<img src=\"cid:pasted-1@example.test\""),
        "{}",
        output.html
    );
    // Never the `data:` URI itself: the body references the part, which is what an Outlook
    // reader renders.
    assert!(!output.html.contains("data:image"), "{}", output.html);
}

#[test]
fn an_attachment_must_name_its_bytes_exactly_once() {
    let mut both = pasted_attachment("img-1", PASTED_PNG);
    both.blob = Some(blob("blob://inline"));
    assert!(matches!(
        render(&pasted_document(both)),
        Err(ComposerError::AttachmentWithTwoSources { .. })
    ));

    let mut neither = pasted_attachment("img-1", PASTED_PNG);
    neither.data_url = None;
    assert!(matches!(
        render(&pasted_document(neither)),
        Err(ComposerError::AttachmentWithoutBytes { .. })
    ));
}

#[test]
fn in_document_bytes_are_only_ever_an_inline_image() {
    // The narrow shape is the gate: a regular file attachment is staged by the host, and a
    // `data:` URI of any other media type would put a document behind an `<img src="cid:">`.
    let mut executable = pasted_attachment("img-1", "data:text/html;base64,PHNjcmlwdD4=");
    executable.media_type = "text/html".to_owned();
    assert!(matches!(
        render(&pasted_document(executable)),
        Err(ComposerError::UnsupportedInlineData { .. })
    ));

    let mut regular = pasted_attachment("img-1", PASTED_PNG);
    regular.disposition = AttachmentDisposition::Attachment;
    assert!(matches!(
        render(&pasted_document(regular)),
        Err(ComposerError::UnsupportedInlineData { .. })
    ));

    // An SVG is an `image/…` and still refused: it is script-capable, and nothing sniffs bytes on
    // the clipboard, so the media type is the only thing standing between a pasted SVG and a
    // `cid:` part an `<img>` renders.
    let mut vector = pasted_attachment("img-1", "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=");
    vector.media_type = "image/svg+xml".to_owned();
    assert!(matches!(
        render(&pasted_document(vector)),
        Err(ComposerError::UnsupportedInlineData { .. })
    ));
}

#[test]
fn shared_blob_handle_is_listed_once_in_the_manifest() {
    // `regular_attachment` always uses the `blob://file` handle, so the two
    // distinct attachment ids below share a single blob.
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph {
            content: vec![text("two attachments, one blob")],
        })],
        attachments: vec![regular_attachment("file-1"), regular_attachment("file-2")],
    };

    let output = render(&document).expect("valid document");

    assert_eq!(output.attachments.len(), 1);
    assert_eq!(output.attachments[0].id, aid("file-1"));
}
