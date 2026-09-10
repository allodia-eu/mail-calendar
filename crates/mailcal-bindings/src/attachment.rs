//! FFI for writing part of the open message, or the whole of it, to a file the host chose.

use std::sync::Arc;

use crate::{MailcalApp, MailcalError, composer::message_ref};

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
