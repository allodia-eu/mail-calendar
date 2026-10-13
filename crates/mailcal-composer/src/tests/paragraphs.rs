//! Headings, alignment, indents, rules and typefaces: what another client's draft carries and the
//! editor now holds, rendered into markup every client reads.

use super::html_doc;
use crate::{
    Align, Block, ComposerDocument, FontFamily, FontSize, Heading, InlineContent, Paragraph, Rule,
    TextRun, render,
};

fn text(value: &str) -> InlineContent {
    InlineContent::Text(TextRun {
        text: value.to_owned(),
        bold: false,
        italic: false,
        underline: false,
        font_size: None,
        font: None,
        color: None,
        highlight: None,
        link: None,
    })
}

fn document(blocks: Vec<Block>) -> ComposerDocument {
    ComposerDocument {
        blocks,
        attachments: Vec::new(),
    }
}

#[test]
fn a_line_carries_its_heading_alignment_and_indent() {
    let heading = Paragraph {
        heading: Some(Heading::H2),
        align: Some(Align::Center),
        ..Paragraph::new(vec![text("Agenda")])
    };
    let indented = Paragraph {
        indent_px: 47,
        ..Paragraph::new(vec![text("Item")])
    };
    let output = render(&document(vec![
        Block::Paragraph(heading),
        Block::Rule(Rule {}),
        Block::Paragraph(indented),
    ]))
    .expect("valid document");
    assert_eq!(
        output.html,
        html_doc(
            "<h2 style=\"text-align:center\">Agenda</h2><hr><div style=\"margin-left:47px\">Item</div>"
        )
    );
    assert_eq!(
        output.plain_text,
        "Agenda\n------------------------------\n    Item"
    );
}

/// An empty block has no height, so a blank line the user left would vanish in every reader.
#[test]
fn a_blank_line_keeps_its_height() {
    let output = render(&document(vec![Block::Paragraph(
        Paragraph::new(Vec::new()),
    )]))
    .expect("valid");
    assert_eq!(output.html, html_doc("<div><br></div>"));
}

/// A line has no spacing in any reader, and a paragraph states its own: Word, which every Windows
/// Outlook renders with, gives a bare `<p>` 14pt above and below, so an Outlook draft saved here
/// came back with its blank lines spread across the page.
#[test]
fn a_line_has_no_spacing_and_a_paragraph_states_its_own() {
    let paragraph = Paragraph {
        spaced: true,
        indent_px: 40,
        ..Paragraph::new(vec![text("Thunderbird")])
    };
    let output = render(&document(vec![
        Block::Paragraph(Paragraph::new(vec![text("Outlook")])),
        Block::Paragraph(paragraph),
    ]))
    .expect("valid document");
    assert_eq!(
        output.html,
        html_doc("<div>Outlook</div><p style=\"margin:0 0 10px;margin-left:40px\">Thunderbird</p>")
    );
}

#[test]
fn a_run_keeps_its_exact_size_and_typeface() {
    let InlineContent::Text(mut run) = text("Comic") else {
        unreachable!()
    };
    run.font_size = Some(FontSize::from_px(21));
    run.font = FontFamily::new("\"Comic Sans MS\", cursive");
    let output = render(&document(vec![Block::Paragraph(Paragraph::new(vec![
        InlineContent::Text(run),
    ]))]))
    .expect("valid document");
    assert!(
        output.html.contains(
            "<span style=\"font-size:21px;font-family:'Comic Sans MS', cursive\">Comic</span>"
        ),
        "{}",
        output.html
    );
}

#[test]
fn the_new_fields_are_absent_from_a_document_that_does_not_use_them() {
    let json = serde_json::to_value(Paragraph::new(Vec::new())).expect("json");
    assert_eq!(json, serde_json::json!({ "content": [] }));
    let rule = serde_json::to_value(Block::Rule(Rule {})).expect("json");
    assert_eq!(rule, serde_json::json!({ "Rule": {} }));
}
