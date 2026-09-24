//! A comparison run over several messages, in a debug build only (`docs/ai.md`, "Training
//! mode"): which messages to answer, a summary per model and variant, and the whole run as one
//! JSON document. Compiled out of a release build, records and all.

use engine_api::MailListRow;
use mailcal_ai::{ComparisonExport, ComparisonSummary, DraftExport, summarise};
use mailcal_app::MessageRef;

use crate::{MailcalApp, app_training::TrainingResult};

/// A message a run answers, with what the window shows of it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TrainingMessage {
    /// The account it is in.
    pub account_id: String,
    /// Its key in that account.
    pub key: String,
    /// Its sender's name, or address when the header carries no name.
    pub from: String,
    /// Its subject.
    pub subject: String,
}

impl From<MailListRow> for TrainingMessage {
    fn from(row: MailListRow) -> Self {
        Self {
            account_id: row.account.as_str().to_owned(),
            key: row.mail.key.as_str().to_owned(),
            from: row
                .mail
                .from_name
                .filter(|name| !name.trim().is_empty())
                .or(row.mail.from_addr)
                .unwrap_or_default(),
            subject: row.mail.subject.unwrap_or_default(),
        }
    }
}

/// One message of a run and every result drafted for it, in the order they were made.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct TrainingMessageResults {
    /// The message answered.
    pub message: TrainingMessage,
    /// Its results.
    pub results: Vec<TrainingResult>,
}

/// How many drafts failed for one reason.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TrainingFailureCount {
    /// Why, in plain words.
    pub failure: String,
    /// How many.
    pub count: u32,
}

/// How one model did under one variant across a run.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TrainingSummary {
    /// The model asked.
    pub model: String,
    /// The variant's name; `None` for the default instructions.
    pub variant: Option<String>,
    /// How many drafts came back.
    pub succeeded: u32,
    /// How many did not, by why.
    pub failed: Vec<TrainingFailureCount>,
    /// The median time of the drafts that came back, in milliseconds.
    pub median_elapsed_ms: Option<u64>,
    /// The tokens read, over every draft the server counted.
    pub prompt_tokens: u64,
    /// The tokens written, over every draft the server counted.
    pub completion_tokens: u64,
}

impl From<ComparisonSummary> for TrainingSummary {
    fn from(line: ComparisonSummary) -> Self {
        Self {
            model: line.model,
            variant: line.variant,
            succeeded: line.succeeded,
            failed: line
                .failed
                .into_iter()
                .map(|(failure, count)| TrainingFailureCount { failure, count })
                .collect(),
            median_elapsed_ms: line.median_elapsed_ms,
            prompt_tokens: line.prompt_tokens,
            completion_tokens: line.completion_tokens,
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// The newest `count` Inbox messages the person answered, in the accounts that draft in a
    /// style, newest first. **Blocking**: it reads the store.
    #[must_use]
    pub fn training_answered_messages(&self, count: u32) -> Vec<TrainingMessage> {
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        self.runtime
            .block_on(self.app.training_answered(count))
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// A summary line for each model and variant among `results`, in the order each first ran.
    #[must_use]
    pub fn training_summary(&self, results: Vec<TrainingResult>) -> Vec<TrainingSummary> {
        let drafts: Vec<_> = results.into_iter().map(Into::into).collect();
        summarise(&drafts).into_iter().map(Into::into).collect()
    }

    /// The run as one JSON document: each message answered with every result and rating, and the
    /// summary. **Blocking**: it reads the messages.
    #[must_use]
    pub fn training_export(&self, messages: Vec<TrainingMessageResults>) -> String {
        let messages = messages
            .into_iter()
            .map(|answered| DraftExport {
                id: None,
                message: MessageRef::from_parts(&answered.message.account_id, answered.message.key)
                    .and_then(|message| self.runtime.block_on(self.app.training_message(&message))),
                drafts: answered.results.into_iter().map(Into::into).collect(),
            })
            .collect();
        ComparisonExport { messages }.to_json()
    }
}

#[cfg(test)]
mod tests {
    use mailcal_ai::{RatedDraft, summarise};

    use super::TrainingSummary;
    use crate::{TrainingResult, records_ai_feedback::TokenUsage};

    fn result(elapsed_ms: u64, failure: Option<&str>) -> TrainingResult {
        TrainingResult {
            model: "mistral-small".to_owned(),
            variant: Some("terse".to_owned()),
            schema_version: 1,
            language: "nl".to_owned(),
            elapsed_ms,
            usage: failure.is_none().then_some(TokenUsage {
                prompt_tokens: 900,
                completion_tokens: 90,
            }),
            reply: "Hoi Marc".to_owned(),
            summary: String::new(),
            tasks: Vec::new(),
            failure: failure.map(str::to_owned),
            rating: None,
        }
    }

    #[test]
    fn a_run_s_results_are_summed_up_per_model_and_variant() {
        let drafts: Vec<RatedDraft> = [
            result(3_000, None),
            result(9_000, Some("the answer could not be read")),
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let summary: Vec<TrainingSummary> =
            summarise(&drafts).into_iter().map(Into::into).collect();

        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].variant.as_deref(), Some("terse"));
        assert_eq!(summary[0].succeeded, 1);
        assert_eq!(summary[0].failed[0].failure, "the answer could not be read");
        assert_eq!(summary[0].failed[0].count, 1);
        assert_eq!(summary[0].median_elapsed_ms, Some(3_000));
        assert_eq!(summary[0].prompt_tokens, 900);
    }
}
