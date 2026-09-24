//! Drafting a reply in the person's writing style.
//!
//! The draft is made from the message being answered (plain text, with whatever history it
//! quotes), the style's guide and passages, the person's own words from the last two messages
//! they sent the same person, their one-line intent, and the plain text of the signature the
//! composer will add, so the body does not repeat it. It is returned, never sent: the host puts it
//! into the open composer above the quote, where the person edits it (`docs/ai.md`).
//!
//! **Blocking**, like the learn: a host calls it off the main thread.

use std::time::{Duration, Instant};

use engine_api::{AccountId, Provider};
use mailcal_account::WritingStyleId;
use mailcal_ai::{
    Draft, DraftContent, DraftRecord, DraftRequest, GatedBackend, ThreadMessage, corpus,
    draft_reply_with,
    wire::{Metering, Usage},
};
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
    /// The catalog locale the app is shown in; the summary and the checklist are written in it.
    pub ui_language: String,
}

/// A drafted reply.
#[derive(Clone, PartialEq)]
pub struct DraftReply {
    /// The draft's id. The composer carries it back on submit (`ai_draft`), which is how a reply
    /// sent from it is kept out of every later learning run.
    pub draft_id: String,
    /// The body.
    pub text: String,
    /// The bracketed gaps in it, for "check the parts in brackets".
    pub gaps: Vec<String>,
    /// What the message being answered asks, in the interface language; empty when none came.
    pub summary: String,
    /// What the person still has to do before sending.
    pub tasks: Vec<mailcal_ai::DraftTask>,
    /// The language it was written in.
    pub language: String,
    /// What the relay charged and the balance after; `None` from an own endpoint.
    pub metering: Option<Metering>,
    /// The tokens the request read and wrote, when the server said.
    pub usage: Option<Usage>,
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

/// A draft as the backend answered it, with what its record needs.
pub(super) struct Made {
    pub(super) draft: Draft,
    pub(super) style: WritingStyleId,
    /// The schema version of the style's guide.
    pub(super) schema_version: u32,
    /// The body of the message answered, as the prompt carried it.
    pub(super) message: String,
    /// How long the request took.
    pub(super) elapsed: Duration,
}

impl Made {
    /// The draft's record, with what it said, under `model` and `variant`.
    pub(super) fn record(&self, model: &str, variant: Option<&str>) -> DraftRecord {
        DraftRecord {
            model: model.to_owned(),
            variant: variant.map(str::to_owned),
            schema_version: self.schema_version,
            language: self.draft.language.clone(),
            elapsed_ms: u64::try_from(self.elapsed.as_millis()).unwrap_or(u64::MAX),
            usage: self.draft.usage,
            content: Some(DraftContent {
                reply: self.draft.text.clone(),
                summary: self.draft.summary.clone(),
                tasks: self.draft.tasks.clone(),
            }),
        }
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
        let made = self.make_draft(request, &backend, None).await?;
        self.note_metering(made.draft.metering);
        let record = made.record(backend.model_label(), None);
        let draft_id = self.writing_style.observed.issue(
            made.style,
            mailcal_ai::draft_plain(&made.draft.text),
            record,
            made.message,
        );
        let draft = made.draft;
        Ok(DraftReply {
            draft_id,
            text: draft.text,
            gaps: draft.gaps,
            summary: draft.summary,
            tasks: draft.tasks,
            language: draft.language,
            metering: draft.metering,
            usage: draft.usage,
        })
    }

    /// Drafts a reply to `request.message` through `backend`, under `instructions` or the default
    /// ones. Asks the gate before reading anything.
    pub(super) async fn make_draft(
        &self,
        request: &ReplyDraftRequest,
        backend: &GatedBackend,
        instructions: Option<&str>,
    ) -> Result<Made, WritingStyleError> {
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

        let started = Instant::now();
        let draft = draft_reply_with(
            &DraftRequest {
                thread: &thread,
                guide: &guide,
                exemplars: &exemplars,
                recipient_messages: &recipient_messages,
                intent: request.intent.as_deref(),
                language: request.language.as_deref(),
                signature: signature.as_deref(),
                ui_language: &request.ui_language,
            },
            backend,
            instructions,
        )?;
        let elapsed = started.elapsed();
        let [answered] = thread;
        Ok(Made {
            draft,
            style: style_id,
            schema_version: guide.schema_version,
            message: answered
                .body
                .chars()
                .take(mailcal_ai::THREAD_CHARS)
                .collect(),
            elapsed,
        })
    }
}
