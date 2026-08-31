//! Text colour and highlight: what reaches the outgoing HTML, and what a malformed value does.

use super::html_doc;
use crate::{
    Block, ComposerDocument, FontSize, InlineContent, Paragraph, TextColor, TextRun, render,
};

fn colour(value: &str) -> TextColor {
    TextColor::new(value).expect("valid colour")
}

fn run(text: &str, color: Option<TextColor>, highlight: Option<TextColor>) -> InlineContent {
    InlineContent::Text(TextRun {
        text: text.to_owned(),
        bold: false,
        italic: false,
        underline: false,
        font_size: None,
        color,
        highlight,
    })
}

fn document(content: Vec<InlineContent>) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph { content })],
        attachments: Vec::new(),
    }
}

fn body_of(document: &ComposerDocument) -> String {
    render(document).expect("renders").html
}

#[test]
fn a_text_colour_becomes_an_inline_style() {
    let html = body_of(&document(vec![run("red", Some(colour("#ff0000")), None)]));
    assert_eq!(
        html,
        html_doc("<p><span style=\"color:#ff0000\">red</span></p>")
    );
}

#[test]
fn a_highlight_becomes_a_background_colour() {
    let html = body_of(&document(vec![run(
        "marked",
        None,
        Some(colour("#ffff00")),
    )]));
    assert_eq!(
        html,
        html_doc("<p><span style=\"background-color:#ffff00\">marked</span></p>")
    );
}

#[test]
fn size_colour_and_highlight_share_one_span() {
    // A run that is large, red and highlighted must not ship three nested wrappers: mail clients
    // that rewrite or flatten CSS are likelier to lose a mark the deeper it is nested.
    let document = document(vec![InlineContent::Text(TextRun {
        text: "all".to_owned(),
        bold: true,
        italic: false,
        underline: false,
        font_size: Some(FontSize::Large),
        color: Some(colour("#0070c0")),
        highlight: Some(colour("#ffff00")),
    })]);
    assert_eq!(
        body_of(&document),
        html_doc(
            "<p><span style=\"font-size:18px;color:#0070c0;background-color:#ffff00\">\
             <strong>all</strong></span></p>"
        )
    );
}

#[test]
fn an_uncoloured_run_emits_no_span() {
    // The common case: colour must not add a wrapper to every run in every message.
    assert_eq!(
        body_of(&document(vec![run("plain", None, None)])),
        html_doc("<p>plain</p>")
    );
}

#[test]
fn colour_has_no_plain_text_rendering() {
    let output =
        render(&document(vec![run("red", Some(colour("#ff0000")), None)])).expect("renders");
    assert_eq!(output.plain_text, "red");
}

#[test]
fn a_malformed_colour_is_dropped_rather_than_failing_the_send() {
    // Lenient at ingestion, like `width_px`: refusing to send a message because one run carries a
    // colour spelling we do not accept trades a cosmetic loss for a functional one.
    let json = r#"{"blocks":[{"Paragraph":{"content":[
        {"Text":{"text":"x","bold":false,"italic":false,"underline":false,
                 "color":"rgb(255,0,0)","highlight":"chartreuse"}}]}}],"attachments":[]}"#;
    let document: ComposerDocument = serde_json::from_str(json).expect("parses");
    assert_eq!(body_of(&document), html_doc("<p>x</p>"));
}

#[test]
fn a_colour_cannot_inject_a_second_declaration_or_escape_the_attribute() {
    // The value is placed straight into `style="…"`, so this is the boundary that keeps a hostile
    // document from adding declarations (or an event handler) of its own.
    let json = r##"{"blocks":[{"Paragraph":{"content":[
        {"Text":{"text":"x","bold":false,"italic":false,"underline":false,
                 "color":"#fff;background:url(http://tracker.test/p.gif)"}}]}}],"attachments":[]}"##;
    let document: ComposerDocument = serde_json::from_str(json).expect("parses");
    let html = body_of(&document);
    assert!(!html.contains("tracker.test"), "{html}");
    assert_eq!(html, html_doc("<p>x</p>"));
}

#[test]
fn a_short_hex_is_expanded_so_the_wire_carries_one_spelling() {
    let json = r##"{"blocks":[{"Paragraph":{"content":[
        {"Text":{"text":"x","bold":false,"italic":false,"underline":false,"color":"#F00"}}]}}],
        "attachments":[]}"##;
    let document: ComposerDocument = serde_json::from_str(json).expect("parses");
    assert_eq!(
        body_of(&document),
        html_doc("<p><span style=\"color:#ff0000\">x</span></p>")
    );
}

#[test]
fn an_absent_colour_key_is_not_an_error() {
    // Every editor emits the key only when there is a colour, so the common document has neither.
    let json = r#"{"blocks":[{"Paragraph":{"content":[
        {"Text":{"text":"x","bold":false,"italic":false,"underline":false}}]}}],"attachments":[]}"#;
    let document: ComposerDocument = serde_json::from_str(json).expect("parses");
    assert_eq!(body_of(&document), html_doc("<p>x</p>"));
}
