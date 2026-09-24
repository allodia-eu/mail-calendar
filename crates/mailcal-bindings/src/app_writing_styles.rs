//! The Writing style FFI methods on [`MailcalApp`] (`docs/ai.md`). Direct methods rather than
//! intents, because each returns a value, as the signature library's do.
//!
//! **Three of them block**: [`sent_corpus_report`](MailcalApp::sent_corpus_report) reads the Sent
//! folder, and [`learn_writing_style`](MailcalApp::learn_writing_style) and
//! [`draft_reply`](MailcalApp::draft_reply) wait for the AI endpoint. A host calls them off the
//! main thread, as it does the Allodia account sync.

use engine_api::AccountId;
use mailcal_app::{LearnRange, MessageRef, ReplyDraftRequest, WritingStyleError};

use crate::{
    MailcalApp,
    records_writing_style::{
        CorpusReport, DraftReply, LearnReport, WritingStyleDetail, WritingStyleFailure,
        WritingStyleSnapshot,
    },
};

fn account(id: &str) -> Result<AccountId, WritingStyleFailure> {
    AccountId::try_from(id).map_err(|_| WritingStyleFailure::NotFound)
}

#[uniffi::export]
impl MailcalApp {
    /// The Writing style surface (pulled after a `Surface::WritingStyle` signal).
    #[must_use]
    pub fn writing_styles(&self) -> WritingStyleSnapshot {
        self.runtime.block_on(self.app.writing_styles()).into()
    }

    /// One style in full, for the reveal and edit screen.
    #[must_use]
    pub fn writing_style_detail(&self, id: String) -> Option<WritingStyleDetail> {
        self.app.writing_style_detail(&id).map(Into::into)
    }

    /// Renames a style. Returns whether the id named one.
    pub fn rename_writing_style(&self, id: String, name: String) -> bool {
        self.app.rename_writing_style(&id, name)
    }

    /// Replaces a style's notes. Returns whether the id named one.
    pub fn update_writing_style_notes(&self, id: String, notes: String) -> bool {
        self.app.update_writing_style_notes(&id, notes)
    }

    /// Forgets a style, and clears it from every account that drafted in it. Returns whether the
    /// id named one.
    pub fn delete_writing_style(&self, id: String) -> bool {
        self.app.delete_writing_style(&id)
    }

    /// Assigns a style to an account, or clears it with `None`.
    pub fn set_account_writing_style(&self, account: String, style: Option<String>) {
        self.app.set_account_writing_style(&account, style);
    }

    /// The style an account drafts in, when it has one.
    #[must_use]
    pub fn resolve_writing_style(&self, account: String) -> Option<String> {
        self.app.resolve_writing_style(&account)
    }

    /// What learning from `account`'s sent mail between `since` and `until` (seconds since the
    /// Unix epoch, either end open) would read and send, without sending anything. **Blocking.**
    ///
    /// # Errors
    ///
    /// Returns [`WritingStyleFailure::NoSentFolder`] when the account has no Sent folder here.
    pub fn sent_corpus_report(
        &self,
        account_id: String,
        since: Option<i64>,
        until: Option<i64>,
    ) -> Result<CorpusReport, WritingStyleFailure> {
        let account_id = account(&account_id)?;
        self.runtime
            .block_on(
                self.app
                    .sent_corpus_report(&account_id, LearnRange { since, until }),
            )
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Learns a style from `account`'s sent mail between `since` and `until`, stores it as `name`
    /// and assigns it to the account when it had none. `ui_language` is the catalog locale the app
    /// is shown in; the description is written in it. **Blocking**; progress arrives on
    /// `Surface::WritingStyle`, and
    /// [`cancel_writing_style_learning`](Self::cancel_writing_style_learning) stops it.
    ///
    /// # Errors
    ///
    /// Returns the [`WritingStyleFailure`] that stopped it.
    pub fn learn_writing_style(
        &self,
        account_id: String,
        since: Option<i64>,
        until: Option<i64>,
        name: String,
        ui_language: String,
    ) -> Result<LearnReport, WritingStyleFailure> {
        let account_id = account(&account_id)?;
        self.runtime
            .block_on(self.app.learn_writing_style(
                &account_id,
                LearnRange { since, until },
                name,
                &ui_language,
            ))
            .map(Into::into)
            .map_err(|failure| failure.error.into())
    }

    /// Stops a learning run before its next request.
    pub fn cancel_writing_style_learning(&self) {
        self.app.cancel_writing_style_learning();
    }

    /// Drafts a reply to the message `key` in `account_id`, for the open composer. `from` is the
    /// account the reply is sent from when the composer changed it; `style` overrides that
    /// account's writing style; `intent` is what the person wants the reply to say; `language`
    /// overrides the language of the message; `ui_language` is the catalog locale the app is shown
    /// in, which the summary and the checklist are written in. **Blocking.** Nothing is sent: the
    /// host inserts the text above the quote, where the person edits it.
    ///
    /// # Errors
    ///
    /// Returns the [`WritingStyleFailure`] that stopped it.
    #[allow(clippy::too_many_arguments)]
    pub fn draft_reply(
        &self,
        account_id: String,
        key: String,
        from: Option<String>,
        style: Option<String>,
        intent: Option<String>,
        language: Option<String>,
        ui_language: String,
    ) -> Result<DraftReply, WritingStyleFailure> {
        let message =
            MessageRef::from_parts(&account_id, key).ok_or(WritingStyleFailure::NotFound)?;
        let from = from.as_deref().map(account).transpose()?;
        self.runtime
            .block_on(self.app.draft_reply(&ReplyDraftRequest {
                message,
                from,
                style,
                intent,
                language,
                ui_language,
            }))
            .map(Into::into)
            .map_err(|error: WritingStyleError| error.into())
    }
}
