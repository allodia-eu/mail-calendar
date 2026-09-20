//! Opening a draft that is already on the server back into a composer.
//!
//! A child of [`super`], whose `impl App` block this one continues.
//!
//! **What makes this more than a read.** The composer it opens will save, and that save
//! supersedes the copy it came from (`docs/drafts.md`), so everything the composer does not
//! open with is something the next save takes out of the user's Drafts folder. That is why
//! the files are staged before the answer is given rather than fetched afterwards, why a
//! staging failure is an error rather than an empty list, and why the composition adopts the
//! stored draft's key here: a resumed composer that saved with nothing to replace would leave
//! the user two drafts and no way to tell which one they are editing.

use engine_api::{Message, Provider};

use super::Composition;
use crate::{
    App, CompositionId, StagedAttachment, mail_compose::join_emails, reference::MessageRef,
};

/// A stored draft, as the composer that resumes it needs to open.
///
/// Deliberately not [`ComposeRequest`](crate::ComposeRequest), which carries a **withdrawn**
/// queued send: that one has no files to open with and no composition to save under, and an
/// empty attachment list on this record would be the claim `docs/sending.md` forbids, that
/// the message had nothing attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftResume {
    /// The account whose Drafts folder the draft was in, and which its saves go back to.
    pub account: String,
    /// The `To` field, comma-joined.
    pub to: String,
    /// The `Cc` field, comma-joined.
    pub cc: String,
    /// The `Bcc` field, comma-joined.
    ///
    /// Only what the stored copy carries. A `Bcc` is not in the bytes a server hands back on
    /// most transports, so a draft saved elsewhere usually resumes without one.
    pub bcc: String,
    /// The subject.
    pub subject: String,
    /// The body, as plain text.
    ///
    /// Plain text because that is what the composer can take without a second markup parser
    /// (see this contract's known gaps). The engine derives it from the HTML when the draft
    /// carries no text part, so an HTML-only draft still opens with its words.
    pub body_text: String,
    /// The files the draft carries, already written into the staging directory the caller
    /// named, ready to be attached exactly as a picked file is.
    pub attachments: Vec<StagedAttachment>,
}

impl<P: Provider> App<P> {
    /// Opens the stored draft `message` into `composition`, so the composer that holds it
    /// saves over that copy rather than beside it.
    ///
    /// `composition` is the host's, minted for the composer it is about to open, exactly as
    /// for a reply or a new message: the core never mints one. Resuming into an id that is
    /// already open replaces what was known about it, which is what a host reusing an id
    /// means by it.
    ///
    /// The draft's files are staged into `staging_directory` before this answers, so the
    /// composer opens holding them and cannot save a copy without them
    /// ([`stage_message_attachments`](Self::stage_message_attachments)).
    ///
    /// # Errors
    ///
    /// A plain error string when the message cannot be resolved, when it is **not a draft**,
    /// when its body cannot be read, or when its files cannot be staged. Nothing is adopted
    /// on any of those: a composition that opened without the draft's content must not be
    /// one whose next save replaces it.
    pub async fn resume_draft(
        &self,
        composition: CompositionId,
        message: MessageRef,
        staging_directory: &str,
    ) -> Result<DraftResume, String> {
        let Some(stored) = self.find_message_in(&message).await else {
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
        let body = self.draft_body_text(&message, &stored).await?;
        let attachments = self
            .stage_attachments_of(&message, &stored, staging_directory)
            .await?;
        let resumed = DraftResume {
            account: message.account.as_str().to_owned(),
            to: join_emails(&stored.envelope.to),
            cc: join_emails(&stored.envelope.cc),
            bcc: join_emails(&stored.envelope.bcc),
            subject: stored.envelope.subject.clone().unwrap_or_default(),
            body_text: body,
            attachments,
        };
        self.adopt_stored_draft(composition, &message, &stored);
        Ok(resumed)
    }

    /// The draft's words, as text.
    async fn draft_body_text(
        &self,
        message: &MessageRef,
        stored: &Message,
    ) -> Result<String, String> {
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
        Ok(body.plain().unwrap_or_default().to_owned())
    }

    /// Joins the composition to the copy already on the server: its key, so the first save
    /// supersedes it and a discard removes it, and its `Message-ID`, so every save of this
    /// composition keeps naming the one message.
    ///
    /// A draft written elsewhere may carry no `Message-ID` at all, and a new one then serves
    /// as well: what the header has to be is the *same* on every save of this composition,
    /// which is what the engine leases a queued save's resource on. It is the key, not the
    /// header, that names the copy being replaced.
    ///
    /// No digest is recorded. The body the composer opens with is text derived from the
    /// stored draft, and re-rendering it produces a different message than the one on the
    /// server, so claiming the two match would skip a save the user can see is needed. The
    /// first save after a resume therefore always writes.
    fn adopt_stored_draft(
        &self,
        composition: CompositionId,
        message: &MessageRef,
        stored: &Message,
    ) {
        let message_id = stored
            .envelope
            .message_id
            .first()
            .cloned()
            .or_else(crate::helpers::new_message_id);
        let Some(message_id) = message_id else {
            // Neither the draft's own header nor a new one: nothing to key the saves on, so
            // the composition is left unknown and the stored copy where it is, rather than
            // handed to a composer whose saves cannot be told apart.
            log::warn!("drafts: the resumed draft has no id to save under");
            return;
        };
        let mut state = self.drafts.lock().expect("drafts mutex poisoned");
        // A composer that has just opened has saved nothing, whatever a composition of this
        // name did before it: the hint is per composer, and one reading "Saved" on a draft
        // this composer has not touched is the state of the one before it
        // (`docs/reading-window.md`).
        state.status.remove(&composition);
        state.open.insert(
            composition,
            Composition {
                account: message.account.clone(),
                message_id,
                key: Some(message.key.clone()),
                saved: None,
                queued: None,
            },
        );
    }
}
