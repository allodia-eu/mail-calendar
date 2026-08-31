//! FFI methods for reading-view attachment downloads.

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
}
