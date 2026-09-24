//! Comparing drafts, in a debug build only (`docs/ai.md`, "Training mode"): one message drafted
//! through a backend the caller builds for each model, under instructions the caller names, and
//! the record of each draft or of why none came back.
//!
//! Nothing here reaches a composer. No draft is issued, so none is remembered for feedback or for
//! the observations log, and no balance is recorded. The backend is a `GatedBackend` like any
//! other, so the gate and the endpoint's declaration apply to every request.

use std::time::Instant;

use engine_api::Provider;
use mailcal_ai::{DraftRecord, GatedBackend};

use super::{ReplyDraftRequest, WritingStyleError};
use crate::{App, MessageRef};

impl<P: Provider> App<P> {
    /// Drafts a reply to `request.message` through `backend`, under the named instructions of
    /// `variant` or the default ones. Answers the draft's record with what it said, or, when none
    /// came back, a record without content, its schema version zero, and why.
    pub async fn training_draft(
        &self,
        request: &ReplyDraftRequest,
        backend: &GatedBackend,
        variant: Option<(&str, &str)>,
    ) -> (DraftRecord, Option<WritingStyleError>) {
        let (name, instructions) = variant.unzip();
        let started = Instant::now();
        match self.make_draft(request, backend, instructions).await {
            Ok(made) => (made.record(backend.model_label(), name), None),
            Err(error) => (
                DraftRecord {
                    model: backend.model_label().to_owned(),
                    variant: name.map(str::to_owned),
                    schema_version: 0,
                    language: String::new(),
                    elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    usage: None,
                    content: None,
                },
                Some(error),
            ),
        }
    }

    /// The plain body of the message a comparison answers, for its export.
    pub async fn training_message(&self, message: &MessageRef) -> Option<String> {
        let original = self.find_message_in(message).await?;
        self.plain_body(&message.account, &original).await
    }
}
