//! The page a message is written on (`docs/reading-dark-mode.md`, "Writing in a dark app").
//!
//! The composer shows the message as the reading view will: on the same canvas, with every colour
//! moved by the same transform. The editor's document keeps the colours it was written with, since
//! that document is what is sent, so it shows each piece moved and asks here how: a run's colour,
//! an attribute or a style sheet inside a quoted original or a signature. The lines this crate's
//! renderer draws itself are on the page from the start.

use core::fmt;

use engine_html::{ColourRole, MessageScheme};
use serde::{Deserialize, Serialize};

use crate::render::{HEADER_RULE, QUOTE_RULE, TABLE_RULE};

/// Which page the message is written on: dark exactly when the reading view would draw it dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageScheme {
    /// The page the message is sent for.
    Light,
    /// A dark page, with the message's colours moved to suit it.
    Dark,
}

impl From<PageScheme> for MessageScheme {
    fn from(scheme: PageScheme) -> Self {
        match scheme {
            PageScheme::Light => Self::Light,
            PageScheme::Dark => Self::Dark,
        }
    }
}

/// The colours the editor draws its page in, each what the reading view shows for the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposerPage {
    /// The scheme these colours are for.
    pub scheme: PageScheme,
    /// The page's fill: the reading view's canvas.
    pub background: String,
    /// Text with no colour of its own.
    pub text: String,
    /// A link with no colour of its own.
    pub link: String,
    /// The indented quote's left border.
    pub quote_rule: String,
    /// The line a line-and-header quote is divided off by.
    pub header_rule: String,
    /// A table's cell borders.
    pub table_rule: String,
}

/// The page for `scheme`.
#[must_use]
pub fn composer_page(scheme: PageScheme) -> ComposerPage {
    let message = MessageScheme::from(scheme);
    let canvas = message.canvas();
    let line = |colour: &str| {
        message
            .adapt_colour(colour, ColourRole::Line)
            .unwrap_or_else(|| colour.to_owned())
    };
    ComposerPage {
        scheme,
        background: canvas.background().to_owned(),
        text: canvas.foreground().to_owned(),
        link: canvas.link().to_owned(),
        quote_rule: line(QUOTE_RULE),
        header_rule: line(HEADER_RULE),
        table_rule: line(TABLE_RULE),
    }
}

/// The script a host runs to put the editor's message on `scheme`'s page: on load, and whenever
/// the app's appearance or the reader's dark-messages choice changes.
#[must_use]
pub fn page_script(scheme: PageScheme) -> String {
    format!("window.setComposerPage({});", page_json(scheme))
}

/// The script a host has its web view run before the editor's document does, so the editor's
/// first frame is already drawn on `scheme`'s page: until the document has loaded, nothing can be
/// handed to it, and the page it starts on is the light one.
#[must_use]
pub fn page_preload_script(scheme: PageScheme) -> String {
    format!("window.composerPage = {};", page_json(scheme))
}

fn page_json(scheme: PageScheme) -> String {
    // A record of `#rrggbb` and `rgb()` strings: serializing it cannot fail.
    serde_json::to_string(&composer_page(scheme)).unwrap_or_else(|_| "null".to_owned())
}

/// One piece of the message the editor shows on its page.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ColourPiece {
    /// A run's text colour, as the editor stamps it.
    Ink(String),
    /// A run's highlight, as the editor stamps it.
    Fill(String),
    /// An attribute in a quoted original or a signature, its value unescaped.
    Attribute {
        /// The element's tag name.
        element: String,
        /// The attribute's name.
        attribute: String,
        /// Its value.
        value: String,
    },
    /// A `<style>` sheet's text in a quoted original or a signature.
    Sheet(String),
}

impl fmt::Debug for ColourPiece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A quoted original's markup is message content: its size, never its text.
        match self {
            Self::Ink(colour) => f.debug_tuple("Ink").field(colour).finish(),
            Self::Fill(colour) => f.debug_tuple("Fill").field(colour).finish(),
            Self::Attribute {
                element,
                attribute,
                value,
            } => f
                .debug_struct("Attribute")
                .field("element", element)
                .field("attribute", attribute)
                .field("value_len", &value.len())
                .finish(),
            Self::Sheet(sheet) => f.debug_struct("Sheet").field("len", &sheet.len()).finish(),
        }
    }
}

/// The pieces whose colours the editor needs for `scheme`'s page.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColoursRequest {
    /// The page they are shown on.
    pub scheme: PageScheme,
    /// The pieces, in the order their answers come back.
    pub pieces: Vec<ColourPiece>,
}

/// Each piece as the reading view draws it on the request's page, in the request's order: the
/// value to show, or `None` where the reading view shows the piece as it was written.
#[must_use]
pub fn page_colours(request: &ColoursRequest) -> Vec<Option<String>> {
    let scheme = MessageScheme::from(request.scheme);
    request
        .pieces
        .iter()
        .map(|piece| match piece {
            ColourPiece::Ink(colour) => scheme.adapt_colour(colour, ColourRole::Ink),
            ColourPiece::Fill(colour) => scheme.adapt_colour(colour, ColourRole::Fill),
            ColourPiece::Attribute {
                element,
                attribute,
                value,
            } => scheme.adapt_attribute(element, attribute, value),
            ColourPiece::Sheet(sheet) => scheme.adapt_style_sheet(sheet),
        })
        .collect()
}
