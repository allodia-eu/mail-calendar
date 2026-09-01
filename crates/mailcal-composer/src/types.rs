use core::fmt;

use serde::{Deserialize, Serialize};

use crate::{color::TextColor, list::List, quote::Quote, signature::Signature};

/// Stable client-generated attachment id used inside the compose document.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttachmentId(String);

impl AttachmentId {
    /// Creates an attachment id when the value is non-empty after trimming.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The id as stored in the document.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AttachmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AttachmentId").field(&self.0).finish()
    }
}

/// RFC 2392 content id for an inline MIME part, without angle brackets.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentId(String);

impl ContentId {
    /// Creates a content id when it is non-empty and cannot inject headers.
    ///
    /// Rejects the same bytes as the engine's `ContentIdHeader`: besides header
    /// injection (`\r \n \0`), an angle bracket would break the `Content-ID:`
    /// boundary, so `<` and `>` are forbidden too.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty()
            || value
                .bytes()
                .any(|b| matches!(b, b'\r' | b'\n' | b'\0' | b'<' | b'>'))
        {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The content id without angle brackets.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ContentId").field(&self.0).finish()
    }
}

/// Host-provided local blob handle for bytes that must be streamed into MIME later.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DraftBlobHandle(String);

impl DraftBlobHandle {
    /// Creates a handle when it is non-empty after trimming.
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The opaque handle value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DraftBlobHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DraftBlobHandle")
            .field(&"<redacted>")
            .finish()
    }
}

/// Whether an attachment is body-referenced inline content or a regular attachment.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttachmentDisposition {
    /// A CID-backed MIME part referenced by an `<img src="cid:...">`.
    Inline {
        /// Content id without angle brackets.
        cid: ContentId,
    },
    /// A normal file attachment.
    Attachment,
}

impl fmt::Debug for AttachmentDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inline { cid } => f.debug_struct("Inline").field("cid", cid).finish(),
            Self::Attachment => f.write_str("Attachment"),
        }
    }
}

/// Attachment metadata plus wherever its bytes are: a host blob handle, or a `data:` URI the
/// editor captured itself.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftAttachment {
    /// Stable id referenced from document nodes.
    pub id: AttachmentId,
    /// Opaque local blob handle supplied by the host, or `None` when [`Self::data_url`] carries
    /// the bytes instead. Exactly one of the two is set; a document where neither or both are
    /// fails validation.
    #[serde(default)]
    pub blob: Option<DraftBlobHandle>,
    /// Suggested filename for MIME metadata.
    pub file_name: String,
    /// Media type such as `image/png` or `application/pdf`.
    pub media_type: String,
    /// Byte size when known.
    pub size: Option<u64>,
    /// Inline CID part or regular attachment.
    pub disposition: AttachmentDisposition,
    /// A base64 `data:image/…` URI carrying the bytes for a picture the editor captured itself: a
    /// paste, or a file a host read for the "show it in the message" answer on a drop. Nothing is
    /// staged behind it, so there is no handle to give; the core decodes it into a `cid:` part on
    /// submit. Only ever set on an [`AttachmentDisposition::Inline`] image.
    #[serde(default)]
    pub data_url: Option<String>,
}

impl fmt::Debug for DraftAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftAttachment")
            .field("id", &self.id)
            .field("blob", &self.blob)
            .field("file_name", &self.file_name)
            .field("media_type", &self.media_type)
            .field("size", &self.size)
            .field("disposition", &self.disposition)
            // A picture out of the user's message: its length, never its bytes.
            .field("data_url_len", &self.data_url.as_ref().map(String::len))
            .finish()
    }
}

/// A constrained font-size set so clients render and emit the same values.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSize {
    /// Small body text.
    Small,
    /// Default body text.
    Normal,
    /// Larger body text.
    Large,
    /// Heading-like body text.
    Huge,
}

impl FontSize {
    /// Pixel value used in outgoing HTML.
    #[must_use]
    pub const fn css_px(self) -> u8 {
        match self {
            Self::Small => 13,
            Self::Normal => 15,
            Self::Large => 18,
            Self::Huge => 24,
        }
    }
}

impl fmt::Debug for FontSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Small => "Small",
            Self::Normal => "Normal",
            Self::Large => "Large",
            Self::Huge => "Huge",
        })
    }
}

/// A text run and its marks.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRun {
    /// Sensitive body text.
    pub text: String,
    /// Bold mark.
    #[serde(default)]
    pub bold: bool,
    /// Italic mark.
    #[serde(default)]
    pub italic: bool,
    /// Underline mark.
    #[serde(default)]
    pub underline: bool,
    /// Optional constrained font size.
    #[serde(default)]
    pub font_size: Option<FontSize>,
    /// Optional text colour. A value [`TextColor`] cannot represent deserializes to `None` rather
    /// than failing the whole document: colour is presentation, and refusing to send a message
    /// over one run's spelling trades a cosmetic loss for a functional one.
    #[serde(default, deserialize_with = "crate::color::deserialize_color")]
    pub color: Option<TextColor>,
    /// Optional highlight (background) colour, on the same terms as `color`.
    #[serde(default, deserialize_with = "crate::color::deserialize_color")]
    pub highlight: Option<TextColor>,
}

impl fmt::Debug for TextRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextRun")
            .field("text_len", &self.text.len())
            .field("bold", &self.bold)
            .field("italic", &self.italic)
            .field("underline", &self.underline)
            .field("font_size", &self.font_size)
            .field("color", &self.color)
            .field("highlight", &self.highlight)
            .finish()
    }
}

/// Inline content inside paragraphs, list items, and table cells.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineContent {
    /// Rich text run.
    Text(TextRun),
    /// Inline image embedded within text flow.
    Image(InlineImage),
}

impl fmt::Debug for InlineContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(run) => f.debug_tuple("Text").field(run).finish(),
            Self::Image(image) => f.debug_tuple("Image").field(image).finish(),
        }
    }
}

/// An inline image that must resolve to a CID attachment.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineImage {
    /// Attachment id whose disposition is `Inline`.
    pub attachment_id: AttachmentId,
    /// Accessible alternative text.
    pub alt_text: String,
    /// Optional display width in CSS pixels. A host (e.g. a native picker) may
    /// supply a value larger than `u16::MAX`; it saturates to `u16::MAX` at
    /// ingestion rather than failing the whole document parse.
    #[serde(default, deserialize_with = "deserialize_clamped_width")]
    pub width_px: Option<u16>,
}

/// Deserializes `width_px` through a wider integer and saturates to `u16::MAX`,
/// so an over-large host width clamps instead of rejecting the entire document.
fn deserialize_clamped_width<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<u64>::deserialize(deserializer)?;
    Ok(raw.map(|value| u16::try_from(value).unwrap_or(u16::MAX)))
}

impl fmt::Debug for InlineImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineImage")
            .field("attachment_id", &self.attachment_id)
            .field("alt_text_len", &self.alt_text.len())
            .field("width_px", &self.width_px)
            .finish()
    }
}

/// A paragraph block.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    /// Inline content in order.
    pub content: Vec<InlineContent>,
}

impl fmt::Debug for Paragraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Paragraph")
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// A table cell.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// Inline cell content.
    pub content: Vec<InlineContent>,
}

impl fmt::Debug for TableCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableCell")
            .field("content_len", &self.content.len())
            .finish()
    }
}

/// A table row.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    /// Cells in this row.
    pub cells: Vec<TableCell>,
}

impl fmt::Debug for TableRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TableRow")
            .field("cells_len", &self.cells.len())
            .finish()
    }
}

/// A rectangular table.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Rows in order. Every row must have the same cell count.
    pub rows: Vec<TableRow>,
}

impl fmt::Debug for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Table")
            .field("rows_len", &self.rows.len())
            .finish()
    }
}

/// A block in the composer document.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// Paragraph block.
    Paragraph(Paragraph),
    /// Bulleted or ordered list, whose items may nest further sub-lists.
    List(List),
    /// Table block.
    Table(Table),
    /// A quoted original message (reply/forward), with its pre-sanitised body and attribution.
    Quote(Quote),
    /// The sender's signature, as a raw HTML fragment sanitised on submit.
    Signature(Signature),
}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Paragraph(p) => f.debug_tuple("Paragraph").field(p).finish(),
            Self::List(list) => f.debug_tuple("List").field(list).finish(),
            Self::Table(t) => f.debug_tuple("Table").field(t).finish(),
            Self::Quote(quote) => f.debug_tuple("Quote").field(quote).finish(),
            Self::Signature(signature) => f.debug_tuple("Signature").field(signature).finish(),
        }
    }
}

/// The shared composer document accepted from native WebView editors.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerDocument {
    /// Body blocks.
    pub blocks: Vec<Block>,
    /// Draft attachments, including CID resources for inline images.
    #[serde(default)]
    pub attachments: Vec<DraftAttachment>,
}

impl fmt::Debug for ComposerDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComposerDocument")
            .field("blocks_len", &self.blocks.len())
            .field("attachments_len", &self.attachments.len())
            .finish()
    }
}

/// Attachment reference emitted after validation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputAttachment {
    /// Stable attachment id.
    pub id: AttachmentId,
    /// Opaque local blob handle, or `None` when the document carries the bytes itself; the caller
    /// then reads the `data:` URI off the source [`DraftAttachment`] with this id.
    pub blob: Option<DraftBlobHandle>,
    /// Suggested filename.
    pub file_name: String,
    /// Media type.
    pub media_type: String,
    /// Byte size when known.
    pub size: Option<u64>,
    /// Content id for inline parts, otherwise `None`.
    pub cid: Option<ContentId>,
}

impl fmt::Debug for OutputAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputAttachment")
            .field("id", &self.id)
            .field("blob", &self.blob)
            .field("file_name", &self.file_name)
            .field("media_type", &self.media_type)
            .field("size", &self.size)
            .field("cid", &self.cid)
            .finish()
    }
}

/// Deterministic send-ready output produced from a validated document.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerOutput {
    /// Email-safe HTML body fragment.
    pub html: String,
    /// Plain-text fallback.
    pub plain_text: String,
    /// CID MIME parts referenced by the HTML body.
    pub inline_attachments: Vec<OutputAttachment>,
    /// Regular MIME attachments.
    pub attachments: Vec<OutputAttachment>,
}

impl fmt::Debug for ComposerOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComposerOutput")
            .field("html_len", &self.html.len())
            .field("plain_text_len", &self.plain_text.len())
            .field("inline_attachments_len", &self.inline_attachments.len())
            .field("attachments_len", &self.attachments.len())
            .finish()
    }
}
