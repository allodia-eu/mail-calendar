//! A comparison run, in a debug build only (`docs/ai.md`, "Training mode"): every message a run
//! answered with its drafts and their ratings, and a summary per model and variant.

use std::{collections::BTreeMap, fmt};

use serde::Serialize;

use crate::{DraftExport, RatedDraft};

/// The shape of [`ComparisonExport::to_json`]; moves when a field changes meaning.
const COMPARISON_VERSION: u32 = 1;

/// One summary line: how one model did under one variant across a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComparisonSummary {
    /// The model asked.
    pub model: String,
    /// The variant's name; `None` for the default instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// How many drafts came back.
    pub succeeded: u32,
    /// How many did not, by why.
    pub failed: BTreeMap<String, u32>,
    /// The median time of the drafts that came back, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_elapsed_ms: Option<u64>,
    /// The tokens read, over every draft the server counted.
    pub prompt_tokens: u64,
    /// The tokens written, over every draft the server counted.
    pub completion_tokens: u64,
}

/// A line of a run's model list: the model, then its options. The one option is
/// `reasoning=<level>`, sent as `reasoning_effort`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelLine<'a> {
    /// The model asked.
    pub model: &'a str,
    /// The reasoning effort asked for, when one is.
    pub reasoning_effort: Option<&'a str>,
}

impl<'a> ModelLine<'a> {
    /// Reads `line`.
    ///
    /// # Errors
    ///
    /// Returns why, in plain words, when the line names no model or carries a word that is not
    /// an option.
    pub fn parse(line: &'a str) -> Result<Self, String> {
        let mut words = line.split_whitespace();
        let model = words.next().ok_or("the line names no model")?;
        let mut reasoning_effort = None;
        for word in words {
            match word.split_once('=') {
                Some(("reasoning", level)) if !level.is_empty() => reasoning_effort = Some(level),
                _ => {
                    return Err(format!(
                        "\"{word}\" is not an option; the one option is reasoning=<level>"
                    ));
                }
            }
        }
        Ok(Self {
            model,
            reasoning_effort,
        })
    }
}

/// A summary line for each model and variant among `drafts`, in the order each first ran.
#[must_use]
pub fn summarise<'a>(drafts: impl IntoIterator<Item = &'a RatedDraft>) -> Vec<ComparisonSummary> {
    let mut lines: Vec<(ComparisonSummary, Vec<u64>)> = Vec::new();
    for draft in drafts {
        let record = &draft.record;
        let at = lines
            .iter()
            .position(|(line, _)| line.model == record.model && line.variant == record.variant)
            .unwrap_or_else(|| {
                lines.push((
                    ComparisonSummary {
                        model: record.model.clone(),
                        variant: record.variant.clone(),
                        succeeded: 0,
                        failed: BTreeMap::new(),
                        median_elapsed_ms: None,
                        prompt_tokens: 0,
                        completion_tokens: 0,
                    },
                    Vec::new(),
                ));
                lines.len() - 1
            });
        let (line, times) = &mut lines[at];
        if let Some(failure) = &draft.failure {
            *line.failed.entry(failure.clone()).or_default() += 1;
        } else {
            line.succeeded += 1;
            times.push(record.elapsed_ms);
        }
        if let Some(usage) = record.usage {
            line.prompt_tokens += usage.prompt_tokens;
            line.completion_tokens += usage.completion_tokens;
        }
    }
    lines
        .into_iter()
        .map(|(line, times)| ComparisonSummary {
            median_elapsed_ms: median(times),
            ..line
        })
        .collect()
}

/// The middle of `values`, or the mean of the middle two; `None` for none.
fn median(mut values: Vec<u64>) -> Option<u64> {
    values.sort_unstable();
    let middle = values.len() / 2;
    match values.len() {
        0 => None,
        len if len % 2 == 1 => Some(values[middle]),
        _ => Some(values[middle - 1].midpoint(values[middle])),
    }
}

/// A whole run: each message answered, with its drafts in the order they were made.
#[derive(Clone, PartialEq)]
pub struct ComparisonExport {
    /// One document per message, each without an id.
    pub messages: Vec<DraftExport>,
}

impl ComparisonExport {
    /// A summary line for each model and variant, over every message.
    #[must_use]
    pub fn summary(&self) -> Vec<ComparisonSummary> {
        summarise(self.messages.iter().flat_map(|message| &message.drafts))
    }

    /// The run as indented JSON: its shape's version, the messages, and the summary.
    #[must_use]
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Versioned<'a> {
            version: u32,
            messages: &'a [DraftExport],
            summary: Vec<ComparisonSummary>,
        }
        serde_json::to_string_pretty(&Versioned {
            version: COMPARISON_VERSION,
            messages: &self.messages,
            summary: self.summary(),
        })
        .unwrap_or_default()
    }
}

impl fmt::Debug for ComparisonExport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComparisonExport")
            .field("messages", &self.messages)
            .finish()
    }
}

#[cfg(test)]
#[path = "comparison_tests.rs"]
mod tests;
