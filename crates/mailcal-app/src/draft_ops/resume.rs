//! Opening a draft that is already on the server back into a composer.
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! **What makes this more than a read.** The composer it opens will save, and that save
//! supersedes the copy it came from (`docs/drafts.md`), so everything the composer does not
//! open with is something the next save takes out of the user's Drafts folder. That is why
//! the body, its pictures and the files are all fetched before the answer is given rather than
//! afterwards, why any of them failing is an error rather than an emptier composer, and why the
//! composition adopts the stored draft's key here: a resumed composer that saved with nothing
//! to replace would leave the user two drafts and no way to tell which one they are editing.

use engine_api::{InlinePart, Message, MessageBody, MessageIdHeader, Provider};

use super::reopen::{Opening, kind_of, threading_of};
use crate::{App, QuoteSource, mail_compose::join_emails, reference::MessageRef};

impl<P: Provider> App<P> {
    /// What a composer reopened on the stored draft `message` opens with, its files staged into
    /// `staging_directory` so it cannot save a copy without them
    /// ([`stage_message_attachments`](Self::stage_message_attachments)).
    ///
    /// # Errors
    ///
    /// A plain error string when the message cannot be resolved, when it is **not a draft**,
    /// when its body or its pictures cannot be read, or when its files cannot be staged.
    pub(super) async fn stored_draft(
        &self,
        message: &MessageRef,
        staging_directory: &str,
    ) -> Result<Opening, String> {
        let Some(stored) = self.find_message_in(message).await else {
            return Err("draft is not available".to_owned());
        };
        // Asked of the message rather than of the folder it was opened from: the engine
        // normalises every transport's own tell onto the one keyword, so this is also the
        // answer in a search result and inside a thread, where there is no Drafts folder to
        // read a role off. Refused rather than allowed, because a composition adopting an
        // ordinary message's key would have its first save rewrite received mail and its
        // discard delete it.
        if !stored.is_draft() {
            return Err("that message is not a draft".to_owned());
        }
        let (body, pictures) = self.draft_content(message, &stored).await?;
        let attachments = self
            .stage_attachments_of(message, &stored, staging_directory)
            .await?;
        // A draft written elsewhere may carry no `Message-ID` at all, and a new one then
        // serves as well: what the header has to be is the *same* on every save of this
        // composition, which is what the engine leases a queued save's resource on. It is
        // the key, not the header, that names the copy being replaced.
        let Some(message_id) = stored
            .envelope
            .message_id
            .first()
            .cloned()
            .or_else(crate::helpers::new_message_id)
        else {
            return Err("the draft has no id to save under".to_owned());
        };
        let envelope = &stored.envelope;
        let threading = threading_of(envelope.in_reply_to.first(), &envelope.references);
        let quote = self.quoted_original_of(message, &stored).await;
        // No digest is recorded. The composer renders its own document, which is not byte for
        // byte the stored one, so claiming the two match would skip a save the user can see
        // is needed. The first save after a resume therefore always writes.
        Ok(Opening {
            account: message.account.clone(),
            message_id,
            key: Some(message.key.clone()),
            kind: kind_of(
                threading.as_ref(),
                envelope.subject.as_deref().unwrap_or_default(),
            ),
            threading,
            pictures,
            to: join_emails(&envelope.to),
            cc: join_emails(&envelope.cc),
            bcc: join_emails(&envelope.bcc),
            subject: envelope.subject.clone().unwrap_or_default(),
            html: body.html().map(str::to_owned),
            text: body.plain().unwrap_or_default().to_owned(),
            quote,
            attachments,
            attachments_failed: false,
            // The body already carries the signature it was written with.
            seeds_signature: false,
        })
    }

    /// The draft's body, and the pictures its HTML addresses by `cid:`.
    ///
    /// The pictures are fetched only when the HTML refers to one, so a draft without any costs
    /// no second read. A failure to read them is an error, not a draft opened without them.
    async fn draft_content(
        &self,
        message: &MessageRef,
        stored: &Message,
    ) -> Result<(MessageBody, Vec<InlinePart>), String> {
        let Some(acct) = self.account_handle(&message.account).await else {
            return Err("account is not connected".to_owned());
        };
        let Some(provider) = acct.providers.first() else {
            return Err("mail provider is not connected".to_owned());
        };
        let body = self
            .engine
            .message_body(provider, &message.account, stored)
            .await
            .map_err(|err| err.to_string())?;
        let pictures = if body.html().is_some_and(|html| html.contains("cid:")) {
            self.engine
                .message_inline_parts(provider, &message.account, stored)
                .await
                .map_err(|err| err.to_string())?
        } else {
            Vec::new()
        };
        Ok((body, pictures))
    }

    /// The message a stored reply or forward quotes, when it is still in the store: found in the
    /// draft's own conversation by the `Message-ID` its headers name, the one it answers or, for a
    /// forward, the last of its chain. The host completes the quote's attribution from it, since a
    /// stored quote carries only the style it was saved in.
    async fn quoted_original_of(
        &self,
        message: &MessageRef,
        stored: &Message,
    ) -> Option<QuoteSource> {
        let envelope = &stored.envelope;
        let wanted: &MessageIdHeader = envelope
            .in_reply_to
            .first()
            .or_else(|| envelope.references.last())?;
        let thread = stored.thread.as_ref()?;
        let rows = self
            .engine
            .mail_on_threads(std::slice::from_ref(&message.account), [thread.id.as_str()])
            .await
            .ok()?;
        let row = rows
            .into_iter()
            .find(|row| row.mail.message_id.as_ref() == Some(wanted))?;
        let original_ref = MessageRef {
            account: message.account.clone(),
            key: row.mail.key,
        };
        let original = self.find_message_in(&original_ref).await?;
        self.quote_source(&original_ref, &original).await
    }
}
