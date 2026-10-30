//! Links: what reaches the outgoing HTML and plain text, and what a hostile target does.

use super::html_doc;
use crate::{Block, ComposerDocument, InlineContent, LinkUrl, Paragraph, TextRun, render};

fn run(text: &str, bold: bool, link: Option<&str>) -> InlineContent {
    InlineContent::Text(TextRun {
        font: None,
        text: text.to_owned(),
        bold,
        italic: false,
        underline: false,
        font_size: None,
        color: None,
        highlight: None,
        link: link.map(|url| LinkUrl::new(url).expect("valid link")),
    })
}

fn document(content: Vec<InlineContent>) -> ComposerDocument {
    ComposerDocument {
        blocks: vec![Block::Paragraph(Paragraph::new(content))],
        attachments: Vec::new(),
    }
}

#[test]
fn a_linked_run_becomes_an_anchor_and_names_its_target_in_the_plain_text() {
    let output = render(&document(vec![
        run("See ", false, None),
        run("the agenda", false, Some("https://example.com/a?b=1&c=2")),
        run(".", false, None),
    ]))
    .expect("renders");
    assert_eq!(
        output.html,
        html_doc("<div>See <a href=\"https://example.com/a?b=1&amp;c=2\">the agenda</a>.</div>")
    );
    assert_eq!(
        output.plain_text,
        "See the agenda <https://example.com/a?b=1&c=2>."
    );
}

#[test]
fn runs_sharing_a_target_are_one_link() {
    // A link whose words carry different marks is still one link, in both parts.
    let output = render(&document(vec![
        run("the ", false, Some("https://example.com")),
        run("new", true, Some("https://example.com")),
        run(" plan", false, Some("https://example.com")),
    ]))
    .expect("renders");
    assert_eq!(
        output.html,
        html_doc("<div><a href=\"https://example.com\">the <strong>new</strong> plan</a></div>")
    );
    assert_eq!(output.plain_text, "the new plan <https://example.com>");
}

#[test]
fn adjacent_links_to_different_targets_stay_apart() {
    let output = render(&document(vec![
        run("one", false, Some("https://one.example")),
        run("two", false, Some("https://two.example")),
    ]))
    .expect("renders");
    assert_eq!(
        output.html,
        html_doc(
            "<div><a href=\"https://one.example\">one</a><a href=\"https://two.example\">two</a></div>"
        )
    );
}

#[test]
fn an_address_shown_as_itself_is_written_once_in_the_plain_text() {
    let output = render(&document(vec![
        run("https://example.com", false, Some("https://example.com")),
        run(" or ", false, None),
        run("a@example.com", false, Some("mailto:a@example.com")),
    ]))
    .expect("renders");
    assert_eq!(output.plain_text, "https://example.com or a@example.com");
}

#[test]
fn a_telephone_link_reaches_the_anchor_and_the_plain_text() {
    // A number is shown by its words and dialled through its target, so the plain-text part
    // carries the target beside them; a reader with no markup can still act on it.
    let output = render(&document(vec![
        run("Call ", false, None),
        run("503-222-3333", false, Some("tel:+15032223333")),
    ]))
    .expect("renders");
    assert_eq!(
        output.html,
        html_doc("<div>Call <a href=\"tel:+15032223333\">503-222-3333</a></div>")
    );
    assert_eq!(output.plain_text, "Call 503-222-3333 <tel:+15032223333>");
}

#[test]
fn a_hostile_target_in_the_document_sends_the_words_without_a_link() {
    // The editor is not the only thing that can write a document, so the target is checked as it
    // is read: a `javascript:` link, or one trying to break out of the attribute, arrives as text.
    let json = r#"{"blocks":[{"Paragraph":{"content":[
        {"Text":{"text":"click","bold":false,"italic":false,"underline":false,"link":"javascript:alert(1)"}},
        {"Text":{"text":"me","bold":false,"italic":false,"underline":false,"link":"https://x\" onclick=\"y"}}
    ]}}]}"#;
    let document: ComposerDocument = serde_json::from_str(json).expect("parses");
    let output = render(&document).expect("renders");
    assert_eq!(output.html, html_doc("<div>clickme</div>"));
    assert_eq!(output.plain_text, "clickme");
}

#[test]
fn a_target_holding_quotes_is_escaped_in_the_attribute() {
    let output = render(&document(vec![run(
        "x",
        false,
        Some("https://example.com/'q'"),
    )]))
    .expect("renders");
    assert_eq!(
        output.html,
        html_doc("<div><a href=\"https://example.com/&#39;q&#39;\">x</a></div>")
    );
}
