//! Drafting a reply in the person's writing style.
//!
//! The draft is made from the message being answered (plain text, with whatever history it
//! quotes), the style's guide and passages, the person's own words from the last two messages
//! they sent the same person, their one-line intent, and the plain text of the signature the
//! composer will add, so the body does not repeat it. It is returned, never sent: the host puts it
//! into the open composer above the quote, where the person edits it (`docs/ai.md`).
//!
//! **Blocking**, like the learn: a host calls it off the main thread.

use engine_api::{AccountId, Provider};
use mailcal_account::WritingStyleId;
use mailcal_ai::{DraftRequest, ThreadMessage, corpus, draft_reply, wire::Metering};
use mailcal_viewmodel::SignatureSlotKind;

use super::{WritingStyleError, exemplars_of, guide_of};
use crate::{App, MessageRef};

/// How many of the person's recent messages to the same recipient a draft reads.
const RECIPIENT_MESSAGES: usize = 2;

/// What to draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftRequest {
    /// The message being answered.
    pub message: MessageRef,
    /// The account the reply is sent from, when the composer changed it from the message's own.
    /// Its style and signature are the ones used.
    pub from: Option<AccountId>,
    /// The style to write in; `None` uses the sending account's.
    pub style: Option<String>,
    /// What the person wants the reply to say.
    pub intent: Option<String>,
    /// The language to answer in; `None` answers in the language of the message.
    pub language: Option<String>,
}

/// A drafted reply.
#[derive(Clone, PartialEq)]
pub struct DraftReply {
    /// The body.
    pub text: String,
    /// The bracketed gaps in it, for "check the parts in brackets".
    pub gaps: Vec<String>,
    /// The language it was written in.
    pub language: String,
    /// What the relay charged and the balance after; `None` from an own endpoint.
    pub metering: Option<Metering>,
}

impl std::fmt::Debug for DraftReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftReply")
            .field("text_len", &self.text.len())
            .field("gaps", &self.gaps.len())
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

impl<P: Provider> App<P> {
    /// Drafts a reply to `request.message`.
    ///
    /// # Errors
    ///
    /// Returns [`WritingStyleError::Unavailable`] with no backend, [`WritingStyleError::NoStyle`]
    /// when there is no style to write in, [`WritingStyleError::NotFound`] when the message is not
    /// on this device, or the backend's error.
    pub async fn draft_reply(
        &self,
        request: &ReplyDraftRequest,
    ) -> Result<DraftReply, WritingStyleError> {
        let backend = self
            .writing_style
            .backend()
            .ok_or(WritingStyleError::Unavailable)?;
        backend
            .check()
            .map_err(|refused| WritingStyleError::Ai(mailcal_ai::AiError::Refused(refused)))?;
        let sender = request.from.as_ref().unwrap_or(&request.message.account);
        let style_id = match &request.style {
            Some(id) => WritingStyleId::new(id.clone()),
            None => self.writing_style.assigned(sender.as_str()),
        }
        .ok_or(WritingStyleError::NoStyle)?;
        let (guide, exemplars) = {
            let library = self.writing_style.library();
            let style = library.get(&style_id).ok_or(WritingStyleError::NoStyle)?;
            (guide_of(style), exemplars_of(style))
        };

        let original = self
            .find_message_in(&request.message)
            .await
            .ok_or(WritingStyleError::NotFound)?;
        let body = self
            .plain_body(&request.message.account, &original)
            .await
            .ok_or(WritingStyleError::NotFound)?;
        let author = original.envelope.from.first();
        let thread = [ThreadMessage {
            from: author.map_or_else(String::new, |address| match address.name.as_deref() {
                Some(name) if !name.trim().is_empty() => format!("{name} <{}>", address.email),
                _ => address.email.clone(),
            }),
            date: original
                .sent_at
                .or(original.received_at)
                .map_or_else(String::new, |instant| instant.to_string()),
            body,
        }];

        let signatures = self
            .signatures
            .lock()
            .expect("signatures mutex poisoned")
            .plain_bodies();
        let recipient_messages: Vec<String> = match author {
            Some(address) => self
                .sent_to(sender, &address.email, RECIPIENT_MESSAGES)
                .await
                .iter()
                .map(|body| corpus::own_text(body, &signatures))
                .filter(|text| !text.is_empty())
                .collect(),
            None => Vec::new(),
        };
        let signature = self
            .resolve_signature(sender.as_str(), SignatureSlotKind::ReplyForward)
            .map(|signature| signature.body_plain);

        let draft = draft_reply(
            &DraftRequest {
                thread: &thread,
                guide: &guide,
                exemplars: &exemplars,
                recipient_messages: &recipient_messages,
                intent: request.intent.as_deref(),
                language: request.language.as_deref(),
                signature: signature.as_deref(),
            },
            &backend,
        )?;
        Ok(DraftReply {
            text: draft.text,
            gaps: draft.gaps,
            language: draft.language,
            metering: draft.metering,
        })
    }
}
