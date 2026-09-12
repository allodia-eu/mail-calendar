//! FFI for writing part of the open message, or the whole of it, to a file the host chose:
//! one attachment, the raw source, or every file a forward is about to carry.
//!
//! Public as a module, unlike its neighbours, because the Linux client consumes this crate as
//! an ordinary Rust dependency rather than through generated bindings, and needs to reach
//! [`message_export_file_name`] by path.

use std::sync::Arc;

use crate::{ComposerFileAttachment, MailcalApp, MailcalError, composer::message_ref};

#[uniffi::export]
impl MailcalApp {
    /// Saves a message attachment to `destination_path`.
    ///
    /// `account` and `key` must come from the opened row, and `attachment_id` from the
    /// current [`crate::ReadingSnapshot`]. The host chooses the destination path (save
    /// panel, app-cache staging file, etc.); Rust writes the decoded bytes directly so
    /// attachment content does not cross FFI.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] when the message reference is malformed, the message
    /// or attachment cannot be resolved, the provider/cache read fails, or the file cannot be
    /// written.
    pub fn save_attachment(
        &self,
        account: String,
        key: String,
        attachment_id: u32,
        destination_path: String,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move {
                app.save_attachment(message, attachment_id, &destination_path)
                    .await
            })
            .map_err(MailcalError::Engine)
    }

    /// Writes the raw RFC 5322 source of a message to `destination_path`.
    ///
    /// `account` and `key` must come from the opened row. The host chooses the destination
    /// (save panel, share-sheet staging file), and Rust writes the bytes directly, so a whole
    /// message never crosses the FFI. Those bytes are what the server delivered, unaltered:
    /// the reading view's sanitised HTML is a rendering, and a file built from it would be a
    /// different message.
    ///
    /// ⚠️ Fetches the source when it is not already cached, so call it **off** the main
    /// thread, as with [`MailcalApp::save_attachment`].
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] when the message reference is malformed, the message
    /// cannot be resolved, the provider/cache read fails, or the file cannot be written.
    pub fn save_message_source(
        &self,
        account: String,
        key: String,
        destination_path: String,
    ) -> Result<(), MailcalError> {
        let message = message_ref(&account, key)?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move { app.save_message_source(message, &destination_path).await })
            .map_err(MailcalError::Engine)
    }

    /// Stages the files the message at `account`/`key` carries into `staging_directory`, for a
    /// **forward** composer to open holding them.
    ///
    /// The result is [`ComposerFileAttachment`]s, the same shape a picked file and a shared one
    /// already take, so by the time the message is sent the three are indistinguishable: the
    /// list is displayed, removable and submitted through the paths that already exist. Which
    /// is the point: a forward carries the message on, files included, and the user can still
    /// take one off before sending.
    ///
    /// `staging_directory` is the host's own private cache, as for an attachment opened from
    /// the reading view; the files are the host's to clean up. Attachment content does not
    /// cross FFI: Rust decodes it from the cached raw source and writes the files.
    ///
    /// Reads and writes every file, so call it off the thread that draws the window.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] when the message reference is malformed, the message
    /// or account cannot be resolved, the provider/cache read fails, or a file cannot be
    /// written. Nothing is staged by halves: on an error the composer opens with no files from
    /// the original, and the host says so rather than leaving the user to notice.
    pub fn stage_forwarded_attachments(
        &self,
        account: String,
        key: String,
        staging_directory: String,
    ) -> Result<Vec<ComposerFileAttachment>, MailcalError> {
        let message = message_ref(&account, key)?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(async move {
                app.stage_forwarded_attachments(message, &staging_directory)
                    .await
            })
            .map(|staged| {
                staged
                    .into_iter()
                    .map(|file| ComposerFileAttachment {
                        path: file.path,
                        file_name: file.file_name,
                        media_type: file.media_type,
                    })
                    .collect()
            })
            .map_err(MailcalError::Engine)
    }
}

/// The file name to offer when exporting a message with this `subject`.
///
/// Pass the subject the client **displays**, so an untitled message exports under whatever
/// that client calls one rather than under a second, English name. The core normalises it into
/// something every filesystem accepts and appends `.eml`.
///
/// Exported rather than written in each client because a subject is free text: it holds
/// slashes, control characters and right-to-left overrides, and five hand-rolled answers to
/// that are five different files, some of them written somewhere nobody chose.
#[uniffi::export]
#[must_use]
pub fn message_export_file_name(subject: String) -> String {
    mailcal_app::export_file_name(&subject)
}
