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
        blob: blob("blob://shared"),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Inline {
            cid: cid("chart-1@example.test"),
        },
    };
    let attached = DraftAttachment {
        id: aid("file-1"),
        blob: blob("blob://shared"),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Attachment,
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
        blob: blob("blob://shared"),
        file_name: "chart.png".to_owned(),
        media_type: "image/png".to_owned(),
        size: Some(42),
        disposition: AttachmentDisposition::Inline {
            cid: cid(content_id),
        },
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
