use core::fmt;
use std::collections::{HashMap, HashSet};

use crate::{
    list::List,
    types::{
        AttachmentDisposition, AttachmentId, ComposerDocument, DraftAttachment, InlineContent,
        InlineImage, Table,
    },
};

/// Result type for composer validation and rendering.
pub type ComposerResult<T> = Result<T, ComposerError>;

/// A deterministic validation error for a composer document.
#[derive(Clone, PartialEq, Eq)]
pub enum ComposerError {
    /// Two attachments use the same id.
    DuplicateAttachmentId {
        /// Duplicate id.
        id: AttachmentId,
    },
    /// Required attachment metadata is blank.
    BlankAttachmentField {
        /// Attachment id.
        id: AttachmentId,
        /// Field name.
        field: &'static str,
    },
    /// An attachment supplies no bytes: neither a host blob handle nor a `data:` URI.
    AttachmentWithoutBytes {
        /// Attachment id.
        id: AttachmentId,
    },
    /// An attachment supplies bytes twice: a host blob handle *and* a `data:` URI.
    AttachmentWithTwoSources {
        /// Attachment id.
        id: AttachmentId,
    },
    /// A `data:` attachment is not an inline base64 PNG, JPEG, GIF or WebP, the only shape the
    /// editor may carry bytes for. A regular file attachment is staged by the host and referenced
    /// by handle; any other media type would attach a document, or a script-capable SVG, behind
    /// an `<img>`.
    UnsupportedInlineData {
        /// Attachment id.
        id: AttachmentId,
    },
    /// Inline image points at an unknown attachment id.
    MissingInlineAttachment {
        /// Missing attachment id.
        id: AttachmentId,
    },
    /// Inline image points at a regular attachment instead of a CID part.
    InlineImageUsesAttachmentDisposition {
        /// Attachment id.
        id: AttachmentId,
    },
    /// An inline CID part exists but no image references it.
    UnusedInlineAttachment {
        /// Attachment id.
        id: AttachmentId,
    },
    /// A table has no rows.
    EmptyTable,
    /// A table row has no cells.
    EmptyTableRow {
        /// Row index.
        row: usize,
    },
    /// A table is not rectangular.
    RaggedTable {
        /// Row index with the wrong cell count.
        row: usize,
        /// Expected cell count.
        expected: usize,
        /// Actual cell count.
        actual: usize,
    },
}

impl fmt::Debug for ComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for ComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateAttachmentId { id } => {
                write!(f, "duplicate attachment id {}", id.as_str())
            }
            Self::BlankAttachmentField { id, field } => {
                write!(f, "attachment {} has a blank {field}", id.as_str())
            }
            Self::AttachmentWithoutBytes { id } => {
                write!(f, "attachment {} carries no bytes", id.as_str())
            }
            Self::AttachmentWithTwoSources { id } => write!(
                f,
                "attachment {} carries both a blob handle and inline data",
                id.as_str()
            ),
            Self::UnsupportedInlineData { id } => write!(
                f,
                "attachment {} carries inline data that is not a showable inline picture",
                id.as_str()
            ),
            Self::MissingInlineAttachment { id } => {
                write!(
                    f,
                    "inline image references missing attachment {}",
                    id.as_str()
                )
            }
            Self::InlineImageUsesAttachmentDisposition { id } => write!(
                f,
                "inline image references non-inline attachment {}",
                id.as_str()
            ),
            Self::UnusedInlineAttachment { id } => {
                write!(f, "inline attachment {} is not referenced", id.as_str())
            }
            Self::EmptyTable => f.write_str("table must contain at least one row"),
            Self::EmptyTableRow { row } => write!(f, "table row {row} has no cells"),
            Self::RaggedTable {
                row,
                expected,
                actual,
            } => write!(
                f,
                "table row {row} has {actual} cells but expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ComposerError {}

pub(crate) fn validate(document: &ComposerDocument) -> ComposerResult<Validated<'_>> {
    let attachments = index_attachments(&document.attachments)?;
    let mut referenced_inline = HashSet::new();

    for block in &document.blocks {
        match block {
            crate::types::Block::Paragraph(paragraph) => {
                validate_inlines(&paragraph.content, &attachments, &mut referenced_inline)?;
            }
            crate::types::Block::List(list) => {
                validate_list(list, &attachments, &mut referenced_inline)?;
            }
            // A quote and a signature each carry an opaque HTML fragment (sanitised by the core on
            // submit) and no draft attachments to cross-check, nothing to validate here.
            crate::types::Block::Quote(_) | crate::types::Block::Signature(_) => {}
            crate::types::Block::Table(table) => {
                validate_table(table)?;
                for row in &table.rows {
                    for cell in &row.cells {
                        validate_inlines(&cell.content, &attachments, &mut referenced_inline)?;
                    }
                }
            }
        }
    }

    for attachment in &document.attachments {
        if matches!(attachment.disposition, AttachmentDisposition::Inline { .. })
            && !referenced_inline.contains(&attachment.id)
        {
            return Err(ComposerError::UnusedInlineAttachment {
                id: attachment.id.clone(),
            });
        }
    }

    Ok(Validated {
        attachments,
        referenced_inline,
    })
}

pub(crate) struct Validated<'a> {
    pub attachments: HashMap<&'a AttachmentId, &'a DraftAttachment>,
    pub referenced_inline: HashSet<&'a AttachmentId>,
}

fn index_attachments(
    attachments: &[DraftAttachment],
) -> ComposerResult<HashMap<&AttachmentId, &DraftAttachment>> {
    let mut indexed = HashMap::with_capacity(attachments.len());
    for attachment in attachments {
        if attachment.file_name.trim().is_empty() {
            return Err(ComposerError::BlankAttachmentField {
                id: attachment.id.clone(),
                field: "file_name",
            });
        }
        if attachment.media_type.trim().is_empty() {
            return Err(ComposerError::BlankAttachmentField {
                id: attachment.id.clone(),
                field: "media_type",
            });
        }
        check_attachment_bytes(attachment)?;
        if indexed.insert(&attachment.id, attachment).is_some() {
            return Err(ComposerError::DuplicateAttachmentId {
                id: attachment.id.clone(),
            });
        }
    }
    Ok(indexed)
}

/// The only `data:` URIs an attachment may carry its bytes in: base64 payloads of the raster set
/// every platform decodes natively, the same closed list the core sniffs a dropped file against.
///
/// SVG is deliberately absent. It is script-capable, so it has no business behind the `<img>` a
/// `cid:` part is rendered by, and `image/…` alone would have let one in through a paste, where
/// nothing sniffs the bytes.
const SHOWABLE_DATA_PREFIXES: [&str; 4] = [
    "data:image/png;base64,",
    "data:image/jpeg;base64,",
    "data:image/gif;base64,",
    "data:image/webp;base64,",
];

/// Every attachment names its bytes exactly once, and the in-document form is narrow.
///
/// A host-staged file arrives as a blob handle; a picture the editor captured (a paste, or the
/// "show it in the message" answer on a drop) arrives as a `data:` URI, because there is no staged
/// file to hand a handle for. Carrying both would leave the core choosing between two sets of
/// bytes for one part, and carrying neither would send an empty attachment.
///
/// The in-document form is restricted to an inline `data:image/…` so a document can never smuggle
/// arbitrary bytes past the host: a regular file attachment is always staged natively, and only an
/// image may be decoded into the `cid:` part an `<img>` points at.
fn check_attachment_bytes(attachment: &DraftAttachment) -> ComposerResult<()> {
    match (&attachment.blob, &attachment.data_url) {
        (Some(_), None) => Ok(()),
        (None, None) => Err(ComposerError::AttachmentWithoutBytes {
            id: attachment.id.clone(),
        }),
        (Some(_), Some(_)) => Err(ComposerError::AttachmentWithTwoSources {
            id: attachment.id.clone(),
        }),
        (None, Some(data_url)) => {
            let inline = matches!(attachment.disposition, AttachmentDisposition::Inline { .. });
            if inline
                && SHOWABLE_DATA_PREFIXES
                    .iter()
                    .any(|p| data_url.starts_with(p))
            {
                Ok(())
            } else {
                Err(ComposerError::UnsupportedInlineData {
                    id: attachment.id.clone(),
                })
            }
        }
    }
}

// Recurse through nested sub-lists so an inline image (CID reference) buried in a
// deeply nested item is validated exactly like a top-level one; nesting must not
// open a hole in the inline-attachment checks.
fn validate_list<'a>(
    list: &'a List,
    attachments: &HashMap<&'a AttachmentId, &'a DraftAttachment>,
    referenced_inline: &mut HashSet<&'a AttachmentId>,
) -> ComposerResult<()> {
    for item in &list.items {
        validate_inlines(&item.content, attachments, referenced_inline)?;
        if let Some(child) = &item.child {
            validate_list(child, attachments, referenced_inline)?;
        }
    }
    Ok(())
}

fn validate_inlines<'a>(
    inlines: &'a [InlineContent],
    attachments: &HashMap<&'a AttachmentId, &'a DraftAttachment>,
    referenced_inline: &mut HashSet<&'a AttachmentId>,
) -> ComposerResult<()> {
    for inline in inlines {
        if let InlineContent::Image(image) = inline {
            validate_inline_image(image, attachments, referenced_inline)?;
        }
    }
    Ok(())
}

fn validate_inline_image<'a>(
    image: &'a InlineImage,
    attachments: &HashMap<&'a AttachmentId, &'a DraftAttachment>,
    referenced_inline: &mut HashSet<&'a AttachmentId>,
) -> ComposerResult<()> {
    let Some(attachment) = attachments.get(&image.attachment_id).copied() else {
        return Err(ComposerError::MissingInlineAttachment {
            id: image.attachment_id.clone(),
        });
    };
    if !matches!(attachment.disposition, AttachmentDisposition::Inline { .. }) {
        return Err(ComposerError::InlineImageUsesAttachmentDisposition {
            id: image.attachment_id.clone(),
        });
    }
    referenced_inline.insert(&image.attachment_id);
    Ok(())
}

fn validate_table(table: &Table) -> ComposerResult<()> {
    let Some(first) = table.rows.first() else {
        return Err(ComposerError::EmptyTable);
    };
    let expected = first.cells.len();
    if expected == 0 {
        return Err(ComposerError::EmptyTableRow { row: 0 });
    }
    for (row, table_row) in table.rows.iter().enumerate().skip(1) {
        let actual = table_row.cells.len();
        if actual == 0 {
            return Err(ComposerError::EmptyTableRow { row });
        }
        if actual != expected {
            return Err(ComposerError::RaggedTable {
                row,
                expected,
                actual,
            });
        }
    }
    Ok(())
}
