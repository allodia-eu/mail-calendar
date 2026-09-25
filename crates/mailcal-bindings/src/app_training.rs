//! Comparing drafts across models and instructions, in a debug build only (`docs/ai.md`,
//! "Training mode"). The whole module, records included, is compiled out of a release build, so
//! no shipped binary or generated binding carries it.
//!
//! Each draft goes through a `GatedBackend` built over the own endpoint with the model named for
//! it, so the gate and the endpoint's declaration apply as they do to every other draft.

use std::sync::atomic::{AtomicU64, Ordering};

use mailcal_ai::{
    DRAFT_INSTRUCTIONS, DRAFT_PLACEHOLDERS, DraftContent, DraftRecord, GatedBackend, ModelLine,
    OwnEndpoint, RatedDraft,
};
use mailcal_app::{MessageRef, ReplyDraftRequest, WritingStyleError};

use crate::{
    MailcalApp,
    ai_transport::AiTransport,
    records_ai_feedback::{DraftRating, TokenUsage},
    records_writing_style::DraftTask,
};

/// Instructions a comparison drafts under, by name.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TrainingVariant {
    /// What the developer calls it.
    pub name: String,
    /// The template, with the placeholders `training_placeholders` lists.
    pub instructions: String,
}

/// One draft of a comparison, and its rating once the developer gives one.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TrainingResult {
    /// The model line asked: the model, and any option with it.
    pub model: String,
    /// The variant's name; `None` for the default instructions.
    pub variant: Option<String>,
    /// The schema version of the style; zero when no draft came back.
    pub schema_version: u32,
    /// The language it was written in; empty when no draft came back.
    pub language: String,
    /// How long the request took, in milliseconds.
    pub elapsed_ms: u64,
    /// The tokens it read and wrote, when the server said.
    pub usage: Option<TokenUsage>,
    /// The reply.
    pub reply: String,
    /// What the message asks.
    pub summary: String,
    /// What is left to do.
    pub tasks: Vec<DraftTask>,
    /// Why no draft came back, in plain words.
    pub failure: Option<String>,
    /// The developer's rating.
    pub rating: Option<DraftRating>,
    /// Whether a stop abandoned it mid-request: no model's failure, and nothing to show.
    pub stopped: bool,
}

/// Bumped by `training_stop`; a comparison's request that sees it move is abandoned.
static STOPS: AtomicU64 = AtomicU64::new(0);

impl TrainingResult {
    fn failed(model: &str, variant: Option<&TrainingVariant>, failure: String) -> Self {
        Self {
            model: model.to_owned(),
            variant: variant.map(|variant| variant.name.clone()),
            schema_version: 0,
            language: String::new(),
            elapsed_ms: 0,
            usage: None,
            reply: String::new(),
            summary: String::new(),
            tasks: Vec::new(),
            failure: Some(failure),
            rating: None,
            stopped: false,
        }
    }
}

impl From<(DraftRecord, Option<WritingStyleError>)> for TrainingResult {
    fn from((record, failure): (DraftRecord, Option<WritingStyleError>)) -> Self {
        let content = record.content.unwrap_or(DraftContent {
            reply: String::new(),
            summary: String::new(),
            tasks: Vec::new(),
        });
        Self {
            model: record.model,
            variant: record.variant,
            schema_version: record.schema_version,
            language: record.language,
            elapsed_ms: record.elapsed_ms,
            usage: record.usage.map(Into::into),
            reply: content.reply,
            summary: content.summary,
            tasks: content.tasks.into_iter().map(Into::into).collect(),
            failure: failure.map(|failure| failure.to_string()),
            rating: None,
            stopped: false,
        }
    }
}

impl From<TrainingResult> for RatedDraft {
    fn from(result: TrainingResult) -> Self {
        let content = result.failure.is_none().then(|| DraftContent {
            reply: result.reply,
            summary: result.summary,
            tasks: result.tasks.into_iter().map(Into::into).collect(),
        });
        Self {
            record: DraftRecord {
                model: result.model,
                variant: result.variant,
                schema_version: result.schema_version,
                language: result.language,
                elapsed_ms: result.elapsed_ms,
                usage: result.usage.map(Into::into),
                content,
            },
            failure: result.failure,
            rating: result.rating.map(Into::into),
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// The default draft instructions, placeholders and all, for the training window to show
    /// and copy.
    #[must_use]
    pub fn training_default_instructions(&self) -> String {
        DRAFT_INSTRUCTIONS.to_owned()
    }

    /// The placeholders a variant may use, each written in braces.
    #[must_use]
    pub fn training_placeholders(&self) -> Vec<String> {
        DRAFT_PLACEHOLDERS.map(str::to_owned).to_vec()
    }

    /// Drafts a reply to the message `key` in `account_id` with the model line `model` on the own
    /// endpoint (`ModelLine`), under `variant` or the default instructions, in the account's own
    /// style. **Blocking.** Nothing is issued to a composer and nothing is sent.
    #[must_use]
    pub fn training_draft(
        &self,
        account_id: String,
        key: String,
        model: String,
        variant: Option<TrainingVariant>,
        ui_language: String,
    ) -> TrainingResult {
        let Some(stored) = self.app.own_ai_endpoint() else {
            return TrainingResult::failed(&model, variant.as_ref(), "no own endpoint".to_owned());
        };
        let line = match ModelLine::parse(&model) {
            Ok(line) => line,
            Err(error) => return TrainingResult::failed(&model, variant.as_ref(), error),
        };
        let key_for_endpoint = self.ai_key.lock().expect("ai key lock").clone();
        let endpoint = match OwnEndpoint::new(
            &stored.base_url,
            key_for_endpoint,
            line.model,
            stored.declared,
        ) {
            Ok(endpoint) => match line.reasoning_effort {
                Some(effort) => endpoint.with_reasoning_effort(effort),
                None => endpoint,
            },
            Err(error) => {
                return TrainingResult::failed(&model, variant.as_ref(), error.to_string());
            }
        };
        let seen = STOPS.load(Ordering::Relaxed);
        let transport = match AiTransport::stoppable(self.runtime.handle().clone(), &STOPS) {
            Ok(transport) => transport,
            Err(error) => return TrainingResult::failed(&model, variant.as_ref(), error),
        };
        let backend = GatedBackend::own_endpoint(
            endpoint,
            Box::new(transport),
            self.app.jurisdiction_mode_source(),
        );
        let Some(message) = MessageRef::from_parts(&account_id, key) else {
            return TrainingResult::failed(&model, variant.as_ref(), "not found".to_owned());
        };
        let request = ReplyDraftRequest {
            message,
            from: None,
            style: None,
            intent: None,
            language: None,
            ui_language,
        };
        let named = variant
            .as_ref()
            .map(|variant| (variant.name.as_str(), variant.instructions.as_str()));
        let mut result: TrainingResult = self
            .runtime
            .block_on(self.app.training_draft(&request, &backend, named))
            .into();
        result.model = model;
        result.stopped = STOPS.load(Ordering::Relaxed) != seen;
        result
    }

    /// Abandons every comparison request in flight; each comes back with `stopped` set.
    pub fn training_stop(&self) {
        STOPS.fetch_add(1, Ordering::Relaxed);
    }
}
