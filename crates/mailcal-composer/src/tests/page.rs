use engine_html::{MessageScheme, RemoteImages, render_document, sanitize};

use super::*;
use crate::{
    BackgroundChoice, ColourPiece, ColoursRequest, HostAnswer, HostRequestKind, PageScheme,
    TextColor, answer_script, composer_page, page_colours, page_preload_script, page_script,
    parse_host_request,
};

/// A quoted original carrying each kind of colour a sender writes.
const QUOTED: &str = "<style>.note{color:#202124;border-bottom:1px solid #dadce0}</style>\
    <table bgcolor=\"#f8f9fa\"><tr><td><p class=\"note\" \
    style=\"color:#333333;background-color:#ffffff\">Still on?</p>\
    <font color=\"#000080\">Alice</font></td></tr></table>";

fn coloured(value: &str, color: &str, highlight: &str) -> InlineContent {
    InlineContent::Text(TextRun {
        font: None,
        text: value.to_owned(),
        bold: false,
        italic: false,
        underline: false,
        font_size: None,
        color: TextColor::new(color),
        highlight: TextColor::new(highlight),
        link: None,
    })
}

fn message() -> ComposerDocument {
    let quote = |style| {
        Block::Quote(Quote {
            style,
            attribution: QuoteAttribution {
                line: "On Monday, Alice wrote:".to_owned(),
                headers: vec![QuoteHeader {
                    label: "From".to_owned(),
                    value: "Alice".to_owned(),
                }],
            },
            body_html: QUOTED.to_owned(),
            body_plain: "Still on?".to_owned(),
        })
    };
    ComposerDocument {
        blocks: vec![
            Block::Paragraph(Paragraph::new(vec![coloured("Yes", "#c00000", "#ffff00")])),
            Block::Table(Table {
                rows: vec![TableRow {
                    cells: vec![TableCell {
                        content: vec![text("cell")],
                    }],
                }],
            }),
            quote(QuoteStyle::Indented),
            quote(QuoteStyle::LineAndHeader),
        ],
        attachments: Vec::new(),
    }
}

/// The sent message as the reading view draws it on a dark page.
fn read_dark(document: &ComposerDocument) -> String {
    let sent = render(document).expect("valid document").html;
    render_document(
        &sanitize(&sent).into_html(),
        RemoteImages::Blocked,
        MessageScheme::Dark,
    )
}

fn answer(piece: ColourPiece) -> String {
    page_colours(&ColoursRequest {
        scheme: PageScheme::Dark,
        pieces: vec![piece],
    })
    .remove(0)
    .expect("the piece moves on a dark page")
}

fn attribute(element: &str, attribute: &str, value: &str) -> ColourPiece {
    ColourPiece::Attribute {
        element: element.to_owned(),
        attribute: attribute.to_owned(),
        value: value.to_owned(),
    }
}

#[test]
fn the_page_shows_each_piece_as_the_reading_view_draws_the_sent_message() {
    let read = read_dark(&message());
    let shown = [
        answer(ColourPiece::Ink("#c00000".to_owned())),
        answer(ColourPiece::Fill("#ffff00".to_owned())),
        answer(attribute(
            "p",
            "style",
            "color:#333333;background-color:#ffffff",
        )),
        answer(attribute("table", "bgcolor", "#f8f9fa")),
        answer(attribute("font", "color", "#000080")),
        answer(ColourPiece::Sheet(
            ".note{color:#202124;border-bottom:1px solid #dadce0}".to_owned(),
        )),
    ];
    for piece in shown {
        assert!(
            read.contains(&piece),
            "the reading view has no {piece}:\n{read}"
        );
    }
}

#[test]
fn the_dark_page_is_the_reading_views_canvas_with_the_renderers_own_lines_moved() {
    let read = read_dark(&message());
    let page = composer_page(PageScheme::Dark);
    let canvas = MessageScheme::Dark.canvas();
    assert_eq!(page.background, canvas.background());
    assert_eq!(page.text, canvas.foreground());
    assert_eq!(page.link, canvas.link());
    for rule in [&page.quote_rule, &page.header_rule, &page.table_rule] {
        assert!(
            read.contains(rule.as_str()),
            "the reading view has no {rule}:\n{read}"
        );
    }
    let light = composer_page(PageScheme::Light);
    assert_ne!(page.quote_rule, light.quote_rule);
    assert_ne!(page.header_rule, light.header_rule);
    assert_ne!(page.table_rule, light.table_rule);
}

#[test]
fn the_light_page_is_the_sent_message_as_written() {
    let sent = render(&message()).expect("valid document").html;
    let page = composer_page(PageScheme::Light);
    let canvas = MessageScheme::Light.canvas();
    assert_eq!(page.background, canvas.background());
    assert_eq!(page.text, canvas.foreground());
    assert_eq!(page.link, canvas.link());
    for rule in [&page.quote_rule, &page.header_rule, &page.table_rule] {
        assert!(
            sent.contains(rule.as_str()),
            "the sent message has no {rule}"
        );
    }
    let unmoved = page_colours(&ColoursRequest {
        scheme: PageScheme::Light,
        pieces: vec![
            ColourPiece::Ink("#000000".to_owned()),
            attribute("td", "bgcolor", "#ffffff"),
        ],
    });
    assert_eq!(unmoved, vec![None, None]);
}

#[test]
fn answers_come_back_in_the_order_asked_and_a_piece_left_as_written_is_none() {
    let answers = page_colours(&ColoursRequest {
        scheme: PageScheme::Dark,
        pieces: vec![
            ColourPiece::Ink("#000000".to_owned()),
            ColourPiece::Ink("#ffffff".to_owned()),
            ColourPiece::Fill("#ffffff".to_owned()),
            attribute("a", "href", "https://example.com/"),
        ],
    });
    assert_eq!(
        answers,
        vec![
            Some("#e3e3e3".to_owned()),
            None,
            Some("#1e1e1e".to_owned()),
            None
        ]
    );
}

#[test]
fn the_page_script_names_the_scheme_and_its_colours() {
    let script = page_script(PageScheme::Dark);
    assert!(
        script.starts_with("window.setComposerPage({\"scheme\":\"dark\","),
        "{script}"
    );
    assert!(script.contains("\"background\":\"#1e1e1e\""), "{script}");
    assert!(script.ends_with("});"), "{script}");
}

#[test]
fn the_page_handed_over_before_the_document_runs_is_the_one_named_after() {
    // The editor draws its first frame on the preload's page and the seed names it again: two
    // different pages would repaint the message as it opens.
    let page = |script: &str, open: &str, close: &str| {
        script
            .strip_prefix(open)
            .and_then(|rest| rest.strip_suffix(close))
            .map(str::to_owned)
    };
    for scheme in [PageScheme::Light, PageScheme::Dark] {
        assert_eq!(
            page(&page_preload_script(scheme), "window.composerPage = ", ";"),
            page(&page_script(scheme), "window.setComposerPage(", ");"),
        );
    }
}

#[test]
fn reads_a_colours_request_and_answers_it_in_order() {
    let message = r##"{"id":4,"request":{"colours":{"scheme":"dark","pieces":[{"ink":"#000000"},{"attribute":{"element":"td","attribute":"bgcolor","value":"#ffffff"}},{"sheet":"p{}"}]}}}"##;
    let request = parse_host_request(message).expect("a colours request");
    let HostRequestKind::Colours(colours) = request.kind else {
        panic!("{request:?}");
    };
    let answers = page_colours(&colours);
    assert_eq!(
        answer_script(request.id, &HostAnswer::Colours(answers)),
        r##"window.answerComposerRequest(4, {"colours":["#e3e3e3","#1e1e1e",null]});"##
    );
}

#[test]
fn a_colours_request_may_carry_a_long_quote_and_nothing_else_may() {
    let sheet = format!("p{{color:#000}}{}", " ".repeat(200 * 1024));
    let colours = format!(
        r#"{{"id":1,"request":{{"colours":{{"scheme":"dark","pieces":[{{"sheet":"{sheet}"}}]}}}}}}"#
    );
    assert!(parse_host_request(&colours).is_some());
    let link = format!(
        r#"{{"id":1,"request":{{"link":{{"text":"{}","address":"","removable":false}}}}}}"#,
        "x".repeat(200 * 1024)
    );
    assert_eq!(parse_host_request(&link), None);
    let too_long = colours.replace(' ', &" ".repeat(6));
    assert_eq!(parse_host_request(&too_long), None);
}

#[test]
fn reads_the_toolbars_choice_of_page() {
    let message = r#"{"id":0,"request":{"background":{"scheme":"light"}}}"#;
    assert_eq!(
        parse_host_request(message).map(|request| request.kind),
        Some(HostRequestKind::Background(BackgroundChoice {
            scheme: PageScheme::Light,
        }))
    );
}

#[test]
fn a_quoted_attribute_is_never_written_to_a_log() {
    let piece = attribute("p", "style", "color:#000;font-family:'Secret'");
    let logged = format!("{piece:?}");
    assert!(!logged.contains("Secret"), "{logged}");
    let sheet = format!("{:?}", ColourPiece::Sheet(".secret{}".to_owned()));
    assert!(!sheet.contains("secret"), "{sheet}");
}
