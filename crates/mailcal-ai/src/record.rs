//! What is said about a draft, and what made it (`docs/ai.md`, "Feedback" and "Training mode"): a
//! rating, the record of the draft it rates, and the one JSON document both leave the app in,
//! whether kept as feedback for Allodia or saved by a developer comparing drafts.
//!
//! **Content is a choice.** A record carries the reply, the summary and the tasks only in its
//! [`DraftRecord::content`], and a document carries the message answered only in its `message`;
//! feedback fills either only when the person ticked the box that says so. Neither type ever holds
//! a header, an address or a style.

use std::fmt;

use serde::Serialize;

use crate::{DraftTask, wire::Usage};

/// The longest comment kept, in characters.
pub const COMMENT_CHARS: usize = 1_000;

/// The shape of [`DraftExport::to_json`]; moves when a field changes meaning.
const EXPORT_VERSION: u32 = 1;

/// Whether a draft was good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Thumbs up.
    Up,
    /// Thumbs down.
    Down,
}

impl Verdict {
    /// What the log calls it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }
}

/// What was wrong with a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// The register did not suit the person or the recipient.
    WrongTone,
    /// Too long or too short.
    WrongLength,
    /// It said something that is in neither the thread nor the person's instructions.
    MadeThingsUp,
    /// It answered something other than what was asked.
    MissedThePoint,
    /// It was written in the wrong language.
    WrongLanguage,
    /// Anything else; the comment says what.
    SomethingElse,
}

/// A verdict on one draft, with what was wrong and anything the person added.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct Rating {
    /// Good or not.
    pub verdict: Verdict,
    /// What was wrong, each once, in the order given.
    pub reasons: Vec<Reason>,
    /// What the person added, trimmed and at most [`COMMENT_CHARS`] long; empty when nothing.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

impl Rating {
    /// A rating with each reason once and the comment trimmed and bounded.
    #[must_use]
    pub fn new(verdict: Verdict, reasons: impl IntoIterator<Item = Reason>, comment: &str) -> Self {
        let mut kept = Vec::new();
        for reason in reasons {
            if !kept.contains(&reason) {
                kept.push(reason);
            }
        }
        Self {
            verdict,
            reasons: kept,
            comment: comment.trim().chars().take(COMMENT_CHARS).collect(),
        }
    }
}

impl fmt::Debug for Rating {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rating")
            .field("verdict", &self.verdict)
            .field("reasons", &self.reasons.len())
            .field("comment_len", &self.comment.len())
            .finish()
    }
}

/// What made a draft and, when it is included, what it said.
#[derive(Clone, PartialEq, Serialize)]
pub struct DraftRecord {
    /// The own endpoint's model name, or `allodia` for the relay, which picks its own.
    pub model: String,
    /// The instructions' name when they were not the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The schema version of the style the draft was written in.
    pub schema_version: u32,
    /// The language it was written in; empty when no draft came back.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub language: String,
    /// How long the request took, in milliseconds.
    pub elapsed_ms: u64,
    /// The tokens it read and wrote, when the server said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// What the draft said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<DraftContent>,
}

impl DraftRecord {
    /// The same record with what the draft said left out.
    #[must_use]
    pub fn without_content(&self) -> Self {
        Self {
            content: None,
            ..self.clone()
        }
    }
}

impl fmt::Debug for DraftRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftRecord")
            .field("language", &self.language)
            .field("elapsed_ms", &self.elapsed_ms)
            .field("has_content", &self.content.is_some())
            .finish_non_exhaustive()
    }
}

/// What a draft said: the reply and what came with it.
#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct DraftContent {
    /// The reply as drafted.
    pub reply: String,
    /// What the message answered asks, as the card showed it.
    pub summary: String,
    /// The checklist, as the card showed it.
    pub tasks: Vec<DraftTask>,
}

impl fmt::Debug for DraftContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftContent")
            .field("reply_len", &self.reply.len())
            .field("tasks", &self.tasks.len())
            .finish_non_exhaustive()
    }
}

/// One draft in a document: its record, why it failed when it did, and its rating when it has one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RatedDraft {
    /// What made it.
    #[serde(flatten)]
    pub record: DraftRecord,
    /// Why no draft came back, as a short label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// What was said about it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<Rating>,
}

/// The one document drafts and ratings leave the app in.
#[derive(Clone, PartialEq, Serialize)]
pub struct DraftExport {
    /// An id for the receiver to recognise a document sent twice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The body of the message answered, as plain text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The drafts, in the order they were made.
    pub drafts: Vec<RatedDraft>,
}

impl DraftExport {
    /// The document as indented JSON, with its shape's version first.
    #[must_use]
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct Versioned<'a> {
            version: u32,
            #[serde(flatten)]
            export: &'a DraftExport,
        }
        serde_json::to_string_pretty(&Versioned {
            version: EXPORT_VERSION,
            export: self,
        })
        .unwrap_or_default()
    }
}

impl fmt::Debug for DraftExport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftExport")
            .field("has_message", &self.message.is_some())
            .field("drafts", &self.drafts)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
