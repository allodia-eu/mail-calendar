//! FFI for writing part of the open message, or the whole of it, to a file the host chose:
//! one attachment, every attachment, or the raw source.
//!
//! Public as a module, unlike its neighbours, because the Linux client consumes this crate as
//! an ordinary Rust dependency rather than through generated bindings, and needs to reach
//! [`message_export_file_name`] by path.

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
            .block_on(|| async move {
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
            .block_on(|| async move { app.save_message_source(message, &destination_path).await })
            .map_err(MailcalError::Engine)
    }

    /// Writes every attachment of a message into `directory_path` and returns the paths it
    /// wrote, in the order [`crate::ReadingSnapshot::attachments`] lists them.
    ///
    /// Each file takes the sender's name made safe for a filesystem, numbered when the folder
    /// already holds that name, so nothing there is written over. All or nothing: a failure
    /// removes what this call wrote. A desktop passes the folder the reader picked; a phone
    /// passes an empty staging folder of its own and shares or copies the paths returned.
    ///
    /// ⚠️ Fetches the source when it is not already cached, so call it **off** the main
    /// thread, as with [`MailcalApp::save_attachment`].
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Engine`] when the message reference is malformed, the message
    /// cannot be resolved, the provider/cache read fails, or a file cannot be written.
    pub fn save_all_attachments(
        &self,
        account: String,
        key: String,
        directory_path: String,
    ) -> Result<Vec<String>, MailcalError> {
        let message = message_ref(&account, key)?;
        let app = Arc::clone(&self.app);
        self.runtime
            .block_on(|| async move { app.save_all_attachments(message, &directory_path).await })
            .map_err(MailcalError::Engine)
    }
}

/// Whether the reading view offers **Save all** above `attachment_count` attachments: from two
/// up, because with one the Save beside it already does exactly this.
#[uniffi::export]
#[must_use]
pub fn attachments_offer_save_all(attachment_count: u32) -> bool {
    mailcal_app::save_all::offers_save_all(attachment_count as usize)
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
