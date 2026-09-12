//! The reading surface's conversions: the open message's body and its attachment rows.
//!
//! A third sibling of `convert`, beside `convert_mailbox` and `convert_settings`, for the reason
//! those two exist: each file stays under the 500-line limit. No FFI macros live here, so the
//! generated bindings are unaffected by the split.

use mailcal_viewmodel::{AttachmentRow as AppAttachmentRow, ReadingSnapshot as AppReading};

use crate::{AttachmentRow, ReadingSnapshot};

impl From<AppReading> for ReadingSnapshot {
    fn from(snapshot: AppReading) -> Self {
        Self {
            key: snapshot.key,
            from: snapshot.from,
            avatar: snapshot.avatar.into(),
            to: snapshot.to,
            cc: snapshot.cc,
            bcc: snapshot.bcc,
            html: snapshot.html,
            plain: snapshot.plain,
            has_remote_images: snapshot.has_remote_images,
            load_error: snapshot.load_error,
            attachments: snapshot
                .attachments
                .into_iter()
                .map(AttachmentRow::from)
                .collect(),
            invitation: snapshot.invitation.map(Into::into),
            pending: snapshot.pending,
        }
    }
}

impl From<AppAttachmentRow> for AttachmentRow {
    fn from(row: AppAttachmentRow) -> Self {
        Self {
            id: row.id,
            file_name: row.file_name,
            media_type: row.media_type,
            size: row.size,
        }
    }
}
