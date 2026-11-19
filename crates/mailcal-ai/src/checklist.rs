//! What a drafted reply comes with besides the reply: a short
//! summary of what the message asks, and a checklist of what the person still has to do before
//! sending. The reply's own placeholders are listed here, not by the model, so each item names the
//! exact text the editor looks for; whether each item is done is decided here too.

use std::{collections::BTreeSet, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// What decides whether an item is done, besides the item itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChecklistProgress {
    /// The gaps still in the reply, as the editor last reported them.
    pub gaps_left: Vec<String>,
    /// How many files were added to the message after the draft went in.
    pub files_added: usize,
    /// The items the person ticked, by their place in the list.
    pub ticked: BTreeSet<usize>,
}

/// Whether each of `tasks` is done, in their order: a fill-in item while its gap is gone from the
/// reply, so an undo that brings it back opens it again; an attach item once as many files have
/// been added since the draft as there are attach items, since which file answers which item
/// cannot be told, or once the person ticked it; anything else once the person ticked it. The card
/// and the question Send asks both read this, so they cannot disagree.
#[must_use]
pub fn tasks_done(tasks: &[DraftTask], progress: &ChecklistProgress) -> Vec<bool> {
    let attach = tasks
        .iter()
        .filter(|task| task.kind == TaskKind::Attach)
        .count();
    tasks
        .iter()
        .enumerate()
        .map(|(at, task)| match task.kind {
            TaskKind::FillIn => !progress.gaps_left.contains(&task.text),
            TaskKind::Attach => progress.files_added >= attach || progress.ticked.contains(&at),
            TaskKind::Do => progress.ticked.contains(&at),
        })
        .collect()
}

/// How many of `tasks` are not done ([`tasks_done`]).
#[must_use]
pub fn tasks_open(tasks: &[DraftTask], progress: &ChecklistProgress) -> usize {
    tasks_done(tasks, progress)
        .into_iter()
        .filter(|done| !done)
        .count()
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
