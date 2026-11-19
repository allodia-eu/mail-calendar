//! Draft a reply on [`MailcalApp`]: the record, the intents and the editor's report. Everything
//! here is read from memory or applied in memory before it returns; a draft is made on a thread
//! of its own.

use engine_api::AccountId;
use mailcal_app::{
    CompositionId,
    ai::{AiQueued, ReplyDraftStart},
};
use mailcal_composer::{HostRequestKind, parse_host_request};

use super::{ComposerReplyDraft, ReplyDraftIntent};
use crate::MailcalApp;

#[uniffi::export]
impl MailcalApp {
    /// Draft a reply for `composition`'s composer, sending from `from` (its From dropdown, as
    /// `submit_composition` takes it; `None` for the account it opened on). `None` while the
    /// assistant is not offered or the composition is not a reply (`docs/ai.md` rule 17). Read
    /// again on `Surface::ReplyDraft` and `Surface::WritingStyle`, and when From changes. Read
    /// from memory, so a UI thread may ask it.
    #[must_use]
    #[uniffi::method(default(from = None))]
    pub fn composer_reply_draft(
        &self,
        composition: String,
        from: Option<String>,
    ) -> Option<ComposerReplyDraft> {
        let composition = CompositionId::new(composition)?;
        let from = from.and_then(|from| AccountId::try_from(from.as_str()).ok());
        let added = |mark| self.composer_files.added_since(&composition, mark);
        self.app
            .composer_reply_draft(&composition, from.as_ref(), &added)
            .map(Into::into)
    }

    /// Hands the core a message the editor posted on its host channel, and answers whether it was
    /// the editor's reply-draft report, which only the core reads. A client hands every message
    /// here as well as to `parse_composer_host_request`, which leaves that one out.
    pub fn composer_reply_draft_report(&self, composition: String, message: String) -> bool {
        let Some(request) = parse_host_request(&message) else {
            return false;
        };
        let HostRequestKind::ReplyDraft(report) = request.kind else {
            return false;
        };
        let Some(composition) = CompositionId::new(composition) else {
            return true;
        };
        let mark = self.composer_files.mark(&composition);
        self.app.take_reply_draft_report(&composition, report, mark);
        true
    }
}

impl MailcalApp {
    /// Handles a [`ReplyDraftIntent`]. Applied before this returns.
    pub(crate) fn dispatch_reply_draft(&self, intent: ReplyDraftIntent) {
        match intent {
            ReplyDraftIntent::Draft {
                composition,
                from,
                instruction,
                language,
                replace,
            } => {
                let Some(composition) = CompositionId::new(composition) else {
                    return;
                };
                let start = ReplyDraftStart {
                    from: from.and_then(|from| AccountId::try_from(from.as_str()).ok()),
                    instruction,
                    language,
                    replace,
                };
                if self.app.draft_reply(&composition, start) == Some(AiQueued::Run) {
                    let app = std::sync::Arc::clone(&self.app);
                    std::thread::spawn(move || app.run_ai_work());
                }
            }
            ReplyDraftIntent::Stop { composition } => {
                if let Some(composition) = CompositionId::new(composition) {
                    self.app.stop_reply_draft(&composition);
                }
            }
            ReplyDraftIntent::AnswerArrived {
                composition,
                replace,
            } => {
                if let Some(composition) = CompositionId::new(composition) {
                    self.app.answer_reply_draft_arrived(&composition, replace);
                }
            }
            ReplyDraftIntent::Tick {
                composition,
                index,
                done,
            } => {
                if let (Some(composition), Ok(index)) =
                    (CompositionId::new(composition), usize::try_from(index))
                {
                    self.app.tick_reply_draft_item(&composition, index, done);
                }
            }
            ReplyDraftIntent::SendQuestionAnswered { composition } => {
                if let Some(composition) = CompositionId::new(composition) {
                    self.app.answer_reply_draft_send_question(&composition);
                }
            }
        }
    }

    /// Whether `submit_composition` must not send `composition` yet: Send's question about the
    /// checklist is due and was not answered (`docs/ai.md` rule 24). The function the record's
    /// `send_question` comes from.
    pub(crate) fn reply_draft_send_due(&self, composition: &CompositionId) -> bool {
        let added = |mark| self.composer_files.added_since(composition, mark);
        self.app
            .reply_draft_send_question(composition, &added)
            .is_some()
    }
}
