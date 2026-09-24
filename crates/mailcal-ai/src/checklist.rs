//! What a drafted reply comes with besides the reply (`docs/ai.md`, "Drafting a reply"): a short
//! summary of what the message asks, and a checklist of what the person still has to do before
//! sending. The reply's own placeholders are listed here, not by the model, so each item names the
//! exact text a client looks for to tick it.

use std::fmt;

use schemars::JsonSchema;
use serde::Deserialize;

/// The longest summary kept, in characters.
const SUMMARY_CHARS: usize = 400;
/// The most items the model's list keeps, and the longest item, in characters.
const TASKS_CAP: usize = 6;
const TASK_CHARS: usize = 160;

/// What the model returns for a draft.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct Answered {
    /// What the message being answered asks of the person, in one or two sentences.
    #[serde(default)]
    pub(crate) summary: String,
    /// The body of the reply.
    pub(crate) reply: String,
    /// What the person still has to do that the reply mentions or needs: files or documents to
    /// attach, and actions elsewhere. Empty when there is nothing.
    #[serde(default)]
    pub(crate) tasks: Vec<AnsweredTask>,
}

/// One item of the model's list.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct AnsweredTask {
    /// "attach" for a file or document to attach to the reply, "do" for anything else.
    pub(crate) kind: String,
    /// The task, short and concrete, starting with a verb.
    pub(crate) text: String,
}

/// One thing to do before sending.
#[derive(Clone, PartialEq, Eq)]
pub struct DraftTask {
    /// What kind of thing it is.
    pub kind: TaskKind,
    /// For [`TaskKind::FillIn`] the placeholder exactly as the reply carries it; otherwise the
    /// task, in the interface language.
    pub text: String,
}

impl DraftTask {
    /// A task of `kind` saying `text`.
    #[must_use]
    pub fn new(kind: TaskKind, text: &str) -> Self {
        Self {
            kind,
            text: text.to_owned(),
        }
    }
}

impl fmt::Debug for DraftTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DraftTask")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

/// What kind of thing a checklist item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    /// A placeholder in the reply to replace; done once it is gone from the reply.
    FillIn,
    /// A file or document to attach.
    Attach,
    /// Anything else to do, such as a change in another system or a question to a colleague.
    Do,
}

/// The checklist: a fill-in item for each of the reply's placeholders, in order, then the model's
/// own items, each on one line and the list cut short.
pub(crate) fn checklist(gaps: &[String], asked: Vec<AnsweredTask>) -> Vec<DraftTask> {
    let fill_in = gaps.iter().map(|gap| DraftTask::new(TaskKind::FillIn, gap));
    let listed = asked
        .into_iter()
        .filter_map(|task| {
            let text = sentence(&task.text, TASK_CHARS);
            let kind = if task.kind.trim().eq_ignore_ascii_case("attach") {
                TaskKind::Attach
            } else {
                TaskKind::Do
            };
            (!text.is_empty()).then_some(DraftTask { kind, text })
        })
        .take(TASKS_CAP);
    fill_in.chain(listed).collect()
}

/// The summary on one line, cut to [`SUMMARY_CHARS`].
pub(crate) fn summary(text: &str) -> String {
    sentence(text, SUMMARY_CHARS)
}

/// `text` on one line, cut to `cap`, starting with a capital: a model writes list items in lower
/// case often enough that the card would look unfinished.
fn sentence(text: &str, cap: usize) -> String {
    let text = bounded(text, cap);
    let mut chars = text.chars();
    chars.next().map_or(text.clone(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn bounded(text: &str, cap: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= cap {
        return flat;
    }
    let mut cut: String = flat.chars().take(cap).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
#[path = "checklist_tests.rs"]
mod tests;
