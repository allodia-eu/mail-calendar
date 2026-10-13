//! The spaces and tabs a user typed, as the outgoing HTML keeps them: a recipient's client
//! collapses a run of spaces and drops one at either end of a line unless the message says
//! otherwise.

use super::{html_doc, text};
use crate::{Block, ComposerDocument, InlineContent, Paragraph, TextRun, render};

fn line(content: Vec<InlineContent>) -> String {
    let document = ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph::new(content))],
        attachments: Vec::new(),
    };
    render(&document).expect("renders").html
}

fn bold(value: &str) -> InlineContent {
    InlineContent::Text(TextRun {
        font: None,
        text: value.to_owned(),
        bold: true,
        italic: false,
        underline: false,
        font_size: None,
        color: None,
        highlight: None,
        link: None,
    })
}

#[test]
fn a_single_space_between_words_stays_an_ordinary_one() {
    assert_eq!(line(vec![text("a b")]), html_doc("<div>a b</div>"));
}

#[test]
fn a_run_of_spaces_alternates_so_none_collapses() {
    assert_eq!(line(vec![text("a  b")]), html_doc("<div>a &nbsp;b</div>"));
    assert_eq!(line(vec![text("a   b")]), html_doc("<div>a &nbsp; b</div>"));
}

#[test]
fn a_space_at_either_end_of_a_line_is_a_hard_one() {
    assert_eq!(
        line(vec![text("  indented")]),
        html_doc("<div>&nbsp; indented</div>")
    );
    assert_eq!(
        line(vec![text("trailing ")]),
        html_doc("<div>trailing&nbsp;</div>")
    );
    assert_eq!(
        line(vec![text("   ")]),
        html_doc("<div>&nbsp; &nbsp;</div>")
    );
}

#[test]
fn spaces_are_counted_across_the_runs_of_one_line() {
    // Two runs, one space each, side by side: a browser would collapse them into one.
    assert_eq!(
        line(vec![text("a "), bold(" b")]),
        html_doc("<div>a <strong>&nbsp;b</strong></div>")
    );
    // A space with formatting of its own between two words is still the one between them.
    assert_eq!(
        line(vec![text("a"), bold(" "), text("b")]),
        html_doc("<div>a<strong> </strong>b</div>")
    );
}

#[test]
fn a_tab_keeps_its_width() {
    assert_eq!(
        line(vec![text("a\tb")]),
        html_doc("<div>a<span style=\"white-space:pre\">\t</span>b</div>")
    );
}
