//! Shared rich-message composer contract for Allodia Mail & Calendar.
//!
//! The platform editors own selection, IME behaviour, undo, and table interaction.
//! This crate owns the durable shape that leaves the editor: a validated document,
//! deterministic outgoing HTML, deterministic `text/plain`, and a manifest that
//! tells MIME assembly which blobs are inline CID resources and which are regular
//! attachments.

mod color;
mod file_meta;
mod list;
mod mailto;
mod quote;
mod render;
mod share;
mod signature;
mod types;
mod validate;

pub use color::TextColor;
pub use file_meta::{safe_file_name, safe_media_type};
pub use list::{List, ListItem, ListKind};
pub use mailto::{MailtoPrefill, parse_mailto};
pub use quote::{Quote, QuoteAttribution, QuoteHeader, QuoteStyle};
pub use render::render;
pub use share::{
    MAX_SHARED_ITEMS, RejectedItem, ShareAttachment, SharePrefill, ShareRejection, ShareRequest,
    SharedItem, prefill_from_share,
};
pub use signature::Signature;
pub use types::{
    AttachmentDisposition, AttachmentId, Block, ComposerDocument, ComposerOutput, ContentId,
    DraftAttachment, DraftBlobHandle, FontSize, InlineContent, InlineImage, OutputAttachment,
    Paragraph, Table, TableCell, TableRow, TextRun,
};
pub use validate::{ComposerError, ComposerResult};

#[cfg(test)]
mod tests;
