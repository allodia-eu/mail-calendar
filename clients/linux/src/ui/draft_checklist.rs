//! A drafted reply's card, decided without a widget (`docs/ai.md`, "Summary and checklist" and
//! "Where they show"): the summary, the checklist in the core's order, what the person has done
//! about it, and whether Send has asked. Held once per composer, so a later draft replaces the card
//! and keeps the answer Send already had. The card that draws it is `super::composer_draft_card`.

use mailcal_bindings::{DraftTask, DraftTaskKind};

use crate::l10n;

/// One item of the checklist. No `Debug`: the text is the person's mail.
pub(crate) struct ChecklistItem {
    pub(crate) kind: DraftTaskKind,
    /// The placeholder exactly as the reply carries it for a fill-in item, otherwise the task.
    pub(crate) text: String,
    pub(crate) ticked: bool,
}

impl ChecklistItem {
    /// What the row says.
    pub(crate) fn title(&self) -> String {
        if self.kind == DraftTaskKind::FillIn {
            l10n::composer_task_fill_in(&self.text)
        } else {
            self.text.clone()
        }
    }
}

#[derive(Default)]
pub(crate) struct DraftChecklist {
    summary: String,
    items: Vec<ChecklistItem>,
    /// Which draft the card is for, so an answer about an earlier one lands nowhere.
    draft: u64,
    /// Whether Send has asked about open items. Once per composer, whatever the answer, so a
    /// later draft keeps it.
    has_asked: bool,
    attachments_at_draft: usize,
}

impl DraftChecklist {
    /// Replaces the card with a draft's, in the core's order.
    pub(crate) fn show(&mut self, summary: &str, tasks: &[DraftTask], attachments: usize) {
        summary.clone_into(&mut self.summary);
        self.items = tasks
            .iter()
            .map(|task| ChecklistItem {
                kind: task.kind,
                text: task.text.clone(),
                ticked: false,
            })
            .collect();
        self.attachments_at_draft = attachments;
        self.draft = self.draft.wrapping_add(1);
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn items(&self) -> &[ChecklistItem] {
        &self.items
    }

    pub(crate) const fn draft(&self) -> u64 {
        self.draft
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.summary.is_empty() && self.items.is_empty()
    }

    /// The placeholders the editor is asked about.
    pub(crate) fn placeholders(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.kind == DraftTaskKind::FillIn)
            .map(|item| item.text.clone())
            .collect()
    }

    /// Whether a fill-in item is still open, which is what keeps the editor being asked.
    pub(crate) fn awaits_placeholders(&self) -> bool {
        self.open_of(DraftTaskKind::FillIn)
    }

    /// Whether an item that ticks itself is still open: a fill-in, which follows the reply, or an
    /// attach item, which follows the files.
    pub(crate) fn follows(&self) -> bool {
        self.awaits_placeholders() || self.open_of(DraftTaskKind::Attach)
    }

    fn open_of(&self, kind: DraftTaskKind) -> bool {
        self.items
            .iter()
            .any(|item| item.kind == kind && !item.ticked)
    }

    /// A fill-in item is ticked exactly while its placeholder is gone from the reply, so one that
    /// comes back, by an undo, opens again.
    pub(crate) fn placeholders_left(&mut self, left: &[String]) {
        for item in &mut self.items {
            if item.kind == DraftTaskKind::FillIn {
                item.ticked = !left.contains(&item.text);
            }
        }
    }

    /// The person's tick. A fill-in item follows the reply alone.
    pub(crate) fn toggle(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index)
            && item.kind != DraftTaskKind::FillIn
        {
            item.ticked = !item.ticked;
        }
    }

    /// Ticks every attach item once as many files have been added since the draft as there are
    /// attach items: which file answers which item cannot be told, so fewer files tick none.
    pub(crate) fn attachments_changed(&mut self, count: usize) {
        let attach = self
            .items
            .iter()
            .filter(|item| item.kind == DraftTaskKind::Attach)
            .count();
        if attach == 0 || count.saturating_sub(self.attachments_at_draft) < attach {
            return;
        }
        for item in &mut self.items {
            if item.kind == DraftTaskKind::Attach {
                item.ticked = true;
            }
        }
    }

    pub(crate) fn open_count(&self) -> usize {
        self.items.iter().filter(|item| !item.ticked).count()
    }

    pub(crate) const fn has_asked(&self) -> bool {
        self.has_asked
    }

    /// Whether Send asks before it sends. It never blocks: either answer ends the asking.
    pub(crate) fn asks_before_send(&self) -> bool {
        !self.has_asked && self.open_count() > 0
    }

    pub(crate) fn send_asked(&mut self) {
        self.has_asked = true;
    }
}

/// The editor call that answers which placeholders are still in the reply. The list travels as a
/// JSON array, so a placeholder holding a quote mark stays data.
pub(crate) fn placeholders_script(placeholders: &[String]) -> String {
    format!(
        "window.composerPlaceholdersLeft({})",
        serde_json::json!(placeholders)
    )
}

/// The editor's answer as JSON, read back into the placeholders it names. Anything else is no
/// answer, so a failed read ticks nothing.
pub(crate) fn placeholders_answer(json: Option<&str>) -> Option<Vec<String>> {
    serde_json::from_str(json?).ok()
}

#[cfg(test)]
#[path = "draft_checklist_tests.rs"]
mod tests;
