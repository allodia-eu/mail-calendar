//! A line of the message, the shape it is set in, and the rule between two of them.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::types::InlineContent;

/// A paragraph block: a line of inline content, or a heading.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    /// Inline content in order.
    pub content: Vec<InlineContent>,
    /// Whether the line is set apart from the next by paragraph spacing. A line is not: it is what
    /// Enter makes in the editor, and how Outlook and Gmail write every line, spacing a message
    /// with blank lines instead. A paragraph is what Thunderbird writes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub spaced: bool,
    /// The heading level, or `None` for body text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<Heading>,
    /// The alignment, or `None` for the start of the line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// How far the line is indented, in CSS pixels. Pixels rather than levels because another
    /// client's draft indents by its own step (Outlook by a quarter or half inch, Thunderbird and
    /// Gmail by 40px), and a draft reopened here keeps the indent it was given.
    #[serde(
        default,
        skip_serializing_if = "is_zero",
        deserialize_with = "deserialize_indent"
    )]
    pub indent_px: u16,
}

impl Paragraph {
    /// The deepest indent kept, in CSS pixels.
    pub const MAX_INDENT_PX: u16 = 400;

    /// A body-text line holding `content`, set flush at the start.
    #[must_use]
    pub fn new(content: Vec<InlineContent>) -> Self {
        Self {
            content,
            spaced: false,
            heading: None,
            align: None,
            indent_px: 0,
        }
    }
}

impl fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Paragraph")
            .field("content_len", &self.content.len())
            .field("spaced", &self.spaced)
            .field("heading", &self.heading)
            .field("align", &self.align)
            .field("indent_px", &self.indent_px)
            .finish()
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if signature"
)]
fn is_zero(value: &u16) -> bool {
    *value == 0
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if signature"
)]
fn is_false(value: &bool) -> bool {
    !*value
}

/// Reads an indent as a number of any width and rounds and clamps it, so a fractional or over-deep
/// one sends as the nearest indent kept rather than failing the document.
fn deserialize_indent<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = f64::deserialize(deserializer)?;
    Ok(crate::font::rounded_px(raw).min(Paragraph::MAX_INDENT_PX))
}

/// A heading level, as HTML's `<h1>` to `<h6>`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Heading {
    /// `<h1>`.
    H1,
    /// `<h2>`.
    H2,
    /// `<h3>`.
    H3,
    /// `<h4>`.
    H4,
    /// `<h5>`.
    H5,
    /// `<h6>`.
    H6,
}

impl Heading {
    /// The element the heading is written as.
    #[must_use]
    pub const fn html_tag(self) -> &'static str {
        match self {
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
            Self::H4 => "h4",
            Self::H5 => "h5",
            Self::H6 => "h6",
        }
    }
}

/// A line's alignment other than the start of the line, which is what `None` means.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Align {
    /// Centred.
    Center,
    /// Set against the end of the line.
    Right,
    /// Spread to both edges.
    Justify,
}

impl Align {
    /// The CSS `text-align` value.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "justify",
        }
    }
}

/// A horizontal rule between two blocks: HTML's `<hr>`. It carries nothing, and is a struct rather
/// than a unit variant so the block serializes as `{"Rule":{}}`, an object like every other block,
/// rather than as a bare string a reader testing for a key would trip over.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Rule {}
