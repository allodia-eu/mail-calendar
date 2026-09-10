//! The original's files on a forward: the attachments the reading view lists are fetched from
//! the message being forwarded and put on the outgoing draft.
//!
//! A child of [`super`], whose `impl App` block this one continues, so that file stays under
//! the 500-line limit.

use engine_api::{DraftAttachment, Message, Provider};

use crate::{App, reference::MessageRef};

impl<P: Provider> App<P> {
    /// The files `original` carries, as parts for a forward to send on.
    ///
    /// The list is the engine's, so it is exactly what the reading view shows the user: a
    /// quoted body's `cid:` images are not in it (those are re-attached inline by
    /// `reattach_quote_cids`) and neither is an invitation's own `text/calendar` part. Each
    /// part keeps the name and media type the sender gave it.
    ///
    /// `None` means the files could not be resolved, and the caller must **not** send. A
    /// forward that quietly drops what it was forwarding is the failure this exists to
    /// prevent: nothing on the sent message says a file is missing, and a send cannot be taken
    /// back, while a failed send can be tried again.
    pub(super) async fn forwarded_attachments(
        &self,
        message: &MessageRef,
        original: &Message,
    ) -> Option<Vec<DraftAttachment>> {
        let account = &message.account;
        let acct = self.account_handle(account).await?;
        let provider = acct.providers.first()?;
        let listed = match self
            .engine
            .message_attachments(provider, account, original)
            .await
        {
            Ok(listed) => listed,
            Err(err) => {
                log::warn!("forward: could not list the original's files: {err}");
                return None;
            }
        };
        let mut parts = Vec::with_capacity(listed.len());
        for attachment in listed {
            let content = match self
                .engine
                .message_attachment(provider, account, original, attachment.id())
                .await
            {
                Ok(Some(content)) => content,
                Ok(None) => {
                    log::warn!("forward: the original no longer carries one of its files");
                    return None;
                }
                Err(err) => {
                    log::warn!("forward: could not read one of the original's files: {err}");
                    return None;
                }
            };
            parts.push(DraftAttachment::attachment(
                attachment.file_name().to_owned(),
                attachment.media_type().to_owned(),
                content.into_bytes(),
            ));
        }
        Some(parts)
    }
}
