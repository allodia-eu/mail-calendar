use super::*;

fn signature_doc() -> ComposerDocument {
    ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: vec![text("See you Thursday.")],
            }),
            Block::Signature(Signature {
                body_html: "<p>Alice Doe<br><em>Allodia</em></p>".to_owned(),
                body_plain: "Alice Doe\nAllodia".to_owned(),
            }),
        ],
        attachments: Vec::new(),
    }
}

#[test]
fn renders_the_signature_inside_its_wrapper_with_the_body_verbatim() {
    let output = render(&signature_doc()).expect("valid document");

    // The fragment is the user's authored HTML: emitted byte-for-byte, never re-escaped (that
    // would show them the tags they wrote). The wrapper matches the editor's region class.
    assert_eq!(
        output.html,
        html_doc(
            "<p>See you Thursday.</p>\
             <div class=\"allodia-signature\">\
             <p>Alice Doe<br><em>Allodia</em></p></div>"
        )
    );
}

#[test]
fn the_plain_text_signature_opens_with_the_rfc_3676_delimiter_including_its_trailing_space() {
    let output = render(&signature_doc()).expect("valid document");

    // "-- " (with the trailing space) is what a reader and a mailing list key trailing-signature
    // detection off, asserting the whole string pins the space a trim would silently eat.
    assert_eq!(
        output.plain_text,
        "See you Thursday.\n-- \nAlice Doe\nAllodia"
    );
}

#[test]
fn a_signature_with_no_plain_text_still_emits_the_delimiter() {
    // An images-only signature has nothing to say in text/plain, but the delimiter still marks
    // where the message ended; otherwise the text part would just stop with no boundary.
    let document = ComposerDocument {
        blocks: vec![Block::Signature(Signature {
            body_html: "<img src=\"data:image/png;base64,iVBORw0KGgo=\" alt=\"logo\">".to_owned(),
            body_plain: String::new(),
        })],
        attachments: Vec::new(),
    };

    let output = render(&document).expect("valid document");
    assert_eq!(output.plain_text, "-- ");
    assert!(output.html.contains("data:image/png;base64,iVBORw0KGgo="));
}

#[test]
fn a_signature_sits_above_the_quoted_original_when_the_editor_put_it_there() {
    // Outlook's placement: the reply text, then the signature, then the quote. Block order is
    // the editor's to decide; the renderer must not reorder it (a signature emitted *below* the
    // quote would read as part of the original message).
    let document = ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: vec![text("Sounds good.")],
            }),
            Block::Signature(Signature {
                body_html: "<p>Alice</p>".to_owned(),
                body_plain: "Alice".to_owned(),
            }),
            Block::Quote(Quote {
                style: QuoteStyle::Indented,
                attribution: QuoteAttribution {
                    line: "On 30 Jun 2026, Bob wrote:".to_owned(),
                    headers: Vec::new(),
                },
                body_html: "<p>Lunch?</p>".to_owned(),
                body_plain: "Lunch?".to_owned(),
            }),
        ],
        attachments: Vec::new(),
    };

    let output = render(&document).expect("valid document");
    let signature_at = output.html.find("allodia-signature").expect("signature");
    let quote_at = output.html.find("blockquote").expect("quote");
    assert!(signature_at < quote_at);
    assert_eq!(
        output.plain_text,
        "Sounds good.\n-- \nAlice\nOn 30 Jun 2026, Bob wrote:\n> Lunch?"
    );
}

#[test]
fn signature_block_round_trips_through_serde() {
    let document = signature_doc();
    let json = serde_json::to_string(&document).expect("serialize");
    let back: ComposerDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(document, back);
}

#[test]
fn the_signature_block_serializes_under_the_tag_the_editor_emits() {
    // A cross-language contract, like `QuoteStyle`'s variant names: the shared
    // `clients/composer/dist/editor.html` emits `{ "Signature": { … } }` from its
    // `.allodia-signature` region. Renaming the variant would silently change the wire tag and
    // every client's signature would arrive as an unparseable block.
    let json = serde_json::to_string(&Block::Signature(Signature {
        body_html: "<p>x</p>".to_owned(),
        body_plain: "x".to_owned(),
    }))
    .expect("serialize");
    assert_eq!(
        json,
        "{\"Signature\":{\"body_html\":\"<p>x</p>\",\"body_plain\":\"x\"}}"
    );
}
