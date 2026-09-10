//! The original's files on a forward: staging what the message carries into a directory the
//! host names, so the composer opens holding them like any other attachment.
//!
//! A child of [`super`], whose `impl App` block this one continues, so that file stays under
//! the 500-line limit.

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use engine_api::Provider;

use crate::{App, protocol::StagedAttachment, reference::MessageRef};

/// Rising counter for the staged files' on-disk names, so two composers open at once, or one
/// message forwarded twice, never write over each other.
static STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

impl<P: Provider> App<P> {
    /// Writes the files `message` carries into `directory` and describes each one, so a
    /// forward composer can open holding them.
    ///
    /// The list is the engine's, so it is exactly what the reading view shows: a quoted body's
    /// inline `cid:` images are not in it (the quote re-attaches those as the `cid:` parts it
    /// references) and neither is an invitation's own `text/calendar` part.
    ///
    /// Staging is all or nothing. Every file is decoded from the one raw source, so in
    /// practice they succeed or fail together, and a composer holding some of what it was
    /// forwarding, with nothing on screen saying which, is the failure this exists to prevent.
    ///
    /// The bytes do not cross FFI: Rust decodes them from the cached raw source and writes the
    /// files directly, and the host reads them back at submit like any file the user picked.
    /// `directory` is the host's own private staging area, and the caller owns what is left in
    /// it, as with an attachment opened from the reading view.
    ///
    /// # Errors
    ///
    /// A plain error string when the message or account cannot be resolved, the provider or
    /// cache read fails, or a file cannot be written.
    pub async fn stage_forwarded_attachments(
        &self,
        message: MessageRef,
        directory: &str,
    ) -> Result<Vec<StagedAttachment>, String> {
        let Some(original) = self.find_message_in(&message).await else {
            return Err("message is not available".to_owned());
        };
        let Some(acct) = self.account_handle(&message.account).await else {
            return Err("account is not connected".to_owned());
        };
        let Some(provider) = acct.providers.first() else {
            return Err("mail provider is not connected".to_owned());
        };
        let listed = self
            .engine
            .message_attachments(provider, &message.account, &original)
            .await
            .map_err(|err| err.to_string())?;
        if listed.is_empty() {
            return Ok(Vec::new());
        }
        std::fs::create_dir_all(directory).map_err(|err| err.to_string())?;
        let mut staged = Vec::with_capacity(listed.len());
        for attachment in listed {
            let content = self
                .engine
                .message_attachment(provider, &message.account, &original, attachment.id())
                .await
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "attachment is not available".to_owned())?;
            // The engine's name has already lost path separators and the dot-only forms, so it
            // is safe to join; the counter in front only keeps two files apart.
            let seq = STAGE_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = Path::new(directory).join(format!("{seq}-{}", attachment.file_name()));
            std::fs::write(&path, content.bytes()).map_err(|err| err.to_string())?;
            staged.push(StagedAttachment {
                path: path.to_string_lossy().into_owned(),
                file_name: attachment.file_name().to_owned(),
                media_type: attachment.media_type().to_owned(),
            });
        }
        log::info!("forward: staged {} file(s) from the original", staged.len());
        Ok(staged)
    }
}
