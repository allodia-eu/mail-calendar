use super::*;

fn quote_headers() -> Vec<QuoteHeader> {
    vec![
        QuoteHeader {
            label: "From".to_owned(),
            value: "Alice <alice@x.test>".to_owned(),
        },
        QuoteHeader {
            label: "Sent".to_owned(),
            value: "30 Jun 2026 14:03".to_owned(),
        },
        QuoteHeader {
            label: "To".to_owned(),
            value: "me@x.test".to_owned(),
        },
        QuoteHeader {
            label: "Subject".to_owned(),
            value: "Lunch".to_owned(),
        },
    ]
}

fn quote_doc(style: QuoteStyle) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                content: vec![text("Sounds good.")],
            }),
            Block::Quote(Quote {
                style,
                attribution: QuoteAttribution {
                    // Carries `<`/`>` so escaping in the attribution context is covered.
                    line: "On 30 Jun 2026 14:03, Alice <alice@x.test> wrote:".to_owned(),
                    headers: quote_headers(),
                },
                body_html: "<p>Are we still on for <strong>lunch</strong>?</p>".to_owned(),
                body_plain: "Are we still on for lunch?".to_owned(),
            }),
        ],
        attachments: Vec::new(),
    }
}

#[test]
fn renders_the_indented_style_as_a_left_bordered_blockquote() {
    let output = render(&quote_doc(QuoteStyle::Indented)).expect("valid document");

    assert_eq!(
        output.html,
        html_doc(
            "<p>Sounds good.</p>\
             <p>On 30 Jun 2026 14:03, Alice &lt;alice@x.test&gt; wrote:</p>\
             <blockquote style=\"margin:0 0 0 0.8ex;border-left:2px solid #cccccc;padding-left:1ex\">\
             <p>Are we still on for <strong>lunch</strong>?</p></blockquote>"
        )
    );
    // The attribution leads, then every quoted line is `> `-prefixed.
    assert_eq!(
        output.plain_text,
        "Sounds good.\nOn 30 Jun 2026 14:03, Alice <alice@x.test> wrote:\n> Are we still on for lunch?"
    );
}

#[test]
fn renders_the_line_and_header_style_with_a_top_border_divider_and_header_block() {
    let output = render(&quote_doc(QuoteStyle::LineAndHeader)).expect("valid document");

    // The divider is a top border in the interop blue-grey (not an <hr>), and labels are
    // bold-with-colon-and-space (`<b>From: </b>value`); what an Outlook reader expects.
    assert_eq!(
        output.html,
        html_doc(
            "<p>Sounds good.</p>\
             <div style=\"padding:3pt 0 0;border-top:1pt solid rgb(181, 196, 223)\"><div>\
             <strong>From: </strong>Alice &lt;alice@x.test&gt;<br>\
             <strong>Sent: </strong>30 Jun 2026 14:03<br>\
             <strong>To: </strong>me@x.test<br>\
             <strong>Subject: </strong>Lunch<br>\
             </div></div><div><p>Are we still on for <strong>lunch</strong>?</p></div>"
        )
    );
    assert_eq!(
        output.plain_text,
        "Sounds good.\n________________________________\n\
         From: Alice <alice@x.test>\nSent: 30 Jun 2026 14:03\nTo: me@x.test\n\
         Subject: Lunch\nAre we still on for lunch?"
    );
}

#[test]
fn quote_body_html_is_emitted_verbatim_not_re_escaped() {
    // The body fragment is the core's pre-sanitised HTML; the composer must not escape it
    // (that would double-encode the original); it appears byte-for-byte in the output.
    let output = render(&quote_doc(QuoteStyle::Indented)).expect("valid document");
    assert!(
        output
            .html
            .contains("<p>Are we still on for <strong>lunch</strong>?</p>")
    );
}

#[test]
fn quote_block_round_trips_through_serde() {
    let document = quote_doc(QuoteStyle::LineAndHeader);
    let json = serde_json::to_string(&document).expect("serialize");
    let back: ComposerDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(document, back);
}

#[test]
fn the_style_serializes_under_its_variant_name_the_editor_and_clients_hardcode() {
    // These two tokens are a cross-language contract, not an implementation detail: the shared
    // `clients/composer/dist/editor.html` switches on `data-quote-style`, and each client's
    // QuoteSeed builder emits the same string into the seed JSON. Renaming a variant silently
    // changes the wire token and would leave every client falling back to the default style.
    let json = serde_json::to_string(&QuoteStyle::Indented).expect("serialize");
    assert_eq!(json, "\"Indented\"");
    let json = serde_json::to_string(&QuoteStyle::LineAndHeader).expect("serialize");
    assert_eq!(json, "\"LineAndHeader\"");
}
