//! Settings → Writing style on [`MailcalApp`] (`docs/ai.md` rules 8 to 16): the page, the
//! learning flow, a style's reveal and detail, what a `WritingStyleIntent` does, and the rule a
//! name field asks before Save is pressed.

use std::sync::Arc;

use mailcal_app::ai::{
    LearningIntent, LearningStep, WritingStyleChange, WritingStyleId, writing_style_name,
};

use crate::{MailcalApp, StyleLearningIntent, WritingStyleIntent};

#[path = "records_writing_style_learning.rs"]
mod learning_records;
#[path = "records_writing_style.rs"]
mod records;
#[path = "records_writing_style_reveal.rs"]
mod reveal_records;

pub use learning_records::{
    StyleLearnRange, StyleLearningAccount, StyleLearningDestination, StyleLearningLanguage,
    StyleLearningReport, StyleLearningReview, StyleLearningStage, StyleLearningTrailStage,
    StyleSentReach, WritingStyleLearning,
};
pub use records::{
    WritingStyleChangeKind, WritingStyleEntry, WritingStyleLearnBlock, WritingStyleOpens,
    WritingStyleOutcome, WritingStylePage, WritingStyleRefusal, WritingStyleResult,
    WritingStyleRun, WritingStyleSlot,
};
pub use reveal_records::{
    WritingStyleCard, WritingStyleDetail, WritingStyleFacts, WritingStyleFrequency,
    WritingStyleHabit, WritingStyleHabits, WritingStyleLanguage, WritingStyleLetter,
    WritingStyleParagraph, WritingStylePhrases, WritingStyleReveal, WritingStyleRevealPage,
    WritingStyleSection, WritingStyleSpan, WritingStyleTopic,
};

fn style_id(id: String) -> Option<WritingStyleId> {
    WritingStyleId::new(id)
}

impl MailcalApp {
    /// Handles `AiIntent::WritingStyle`. Returns at once: a library change runs on a thread of
    /// its own, a review's read on the runtime and a run on the assistant's queue.
    pub(crate) fn dispatch_writing_style(&self, intent: WritingStyleIntent) {
        let change = match intent {
            WritingStyleIntent::Learning { intent } => return self.dispatch_learning(intent),
            WritingStyleIntent::Rename { style, name } => {
                style_id(style).map(|id| WritingStyleChange::Rename { id, name })
            }
            WritingStyleIntent::SetNotes { style, notes } => {
                style_id(style).map(|id| WritingStyleChange::Notes { id, notes })
            }
            WritingStyleIntent::Assign { account, style } => Some(WritingStyleChange::Assign {
                account,
                style: style.and_then(style_id),
            }),
            WritingStyleIntent::Forget { style } => {
                style_id(style).map(|id| WritingStyleChange::Forget { id })
            }
            WritingStyleIntent::SaveReveal { style, name, notes } => {
                style_id(style).map(|id| WritingStyleChange::SaveReveal { id, name, notes })
            }
            WritingStyleIntent::CloseReveal { style } => {
                style_id(style).map(|id| WritingStyleChange::CloseReveal { id })
            }
        };
        if let Some(change) = change {
            self.change_writing_style(change);
        }
    }

    fn change_writing_style(&self, change: WritingStyleChange) {
        if self.app.queue_writing_style_change(change) {
            let app = Arc::clone(&self.app);
            std::thread::spawn(move || app.run_writing_style_changes());
        }
    }

    fn dispatch_learning(&self, intent: StyleLearningIntent) {
        let intent = match intent {
            StyleLearningIntent::Open => LearningIntent::Open,
            StyleLearningIntent::SelectAccount { account } => {
                LearningIntent::SelectAccount { account }
            }
            StyleLearningIntent::SelectRange { range } => {
                let Some(range) = range.to_core() else {
                    return;
                };
                LearningIntent::SelectRange { range }
            }
            StyleLearningIntent::Next => LearningIntent::Next,
            StyleLearningIntent::Back => LearningIntent::Back,
            StyleLearningIntent::Learn { name, language } => {
                LearningIntent::Learn { name, language }
            }
            StyleLearningIntent::Stop => LearningIntent::Stop,
            StyleLearningIntent::Remedy => LearningIntent::Remedy,
            StyleLearningIntent::Close => LearningIntent::Close,
        };
        match self.app.writing_style_learning_intent(intent) {
            LearningStep::Nothing => {}
            LearningStep::Read(ticket) => {
                let app = Arc::clone(&self.app);
                self.runtime
                    .spawn(async move { app.read_learning_review(ticket).await });
            }
            LearningStep::Learn(work) => {
                self.queue_ai_work(work);
            }
            LearningStep::Change(change) => self.change_writing_style(change),
        }
    }
}

#[uniffi::export]
impl MailcalApp {
    /// Settings → Writing style, or `None` while the assistant is not offered (pulled after
    /// `Surface::WritingStyle` and `Surface::Settings`). Read from memory.
    #[must_use]
    pub fn writing_style_page(&self) -> Option<WritingStylePage> {
        self.app.writing_style_page().map(records::page)
    }

    /// The learning flow while a host draws it, or `None` (pulled after `Surface::WritingStyle`).
    /// Read from memory.
    #[must_use]
    pub fn writing_style_learning(&self) -> Option<WritingStyleLearning> {
        self.app
            .writing_style_learning()
            .map(learning_records::learning)
    }

    /// The reveal of `style`, or `None` when it names none (pulled after
    /// `Surface::WritingStyle`). Read from memory.
    #[must_use]
    pub fn writing_style_reveal(&self, style: String) -> Option<WritingStyleReveal> {
        let id = style_id(style)?;
        self.app
            .writing_style_reveal(&id)
            .map(reveal_records::reveal)
    }

    /// The detail of `style`, or `None` when it names none (pulled after
    /// `Surface::WritingStyle`). Read from memory.
    #[must_use]
    pub fn writing_style_detail(&self, style: String) -> Option<WritingStyleDetail> {
        let id = style_id(style)?;
        self.app
            .writing_style_detail(&id)
            .map(reveal_records::detail)
    }
}

/// Why a style named `name` would not be saved, or `None` when it would: the reveal's Save and
/// the detail's name hold their Save while this refuses, and the change refuses by the same rule.
#[uniffi::export]
#[must_use]
pub fn writing_style_name_refusal(name: String) -> Option<WritingStyleRefusal> {
    writing_style_name(&name).err().map(Into::into)
}

#[cfg(test)]
#[path = "writing_style_tests.rs"]
mod tests;
