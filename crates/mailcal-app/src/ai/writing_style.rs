//! Settings → Writing style (`docs/ai.md` rules 8 to 16): the page, the learning flow, and a
//! style's reveal and detail, each a record the core holds and a client draws.

use std::sync::Mutex;

use engine_api::Provider;
use mailcal_account::WritingStyleId;
use mailcal_ai::StyleGuide;

mod flow;
mod learning;
mod range;
mod reveal;
mod sent;
mod work;

pub use flow::{LearningIntent, LearningTrailStage, Review};
pub use learning::{LearnAccount, LearningStage, LearningStep, LearningView};
pub use range::{LearnRange, LearnRanges, SentReach, learn_ranges};
pub use reveal::{StyleFacts, WritingStyleDetailView, WritingStyleRevealView};
pub use work::{
    WritingStyleChange, WritingStyleChangeKind, WritingStyleOutcome, WritingStyleRefusal,
    WritingStyleResult, WritingStyleWork, writing_style_name,
};

use super::{AiFailure, RemedyControl};
use crate::App;

/// What a control on these surfaces is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleControl {
    /// It can be acted on now.
    pub enabled: bool,
    /// Work it started is running. A running control is never enabled.
    pub running: bool,
}

impl StyleControl {
    /// A control held while `busy`, running while `running`.
    pub(crate) const fn of(busy: bool, running: bool) -> Self {
        Self {
            enabled: !busy,
            running,
        }
    }
}

/// The library's changes and the learning flow.
#[derive(Default)]
pub(crate) struct WritingStyleState {
    work: Mutex<work::Work>,
    flows: Mutex<flow::Flows>,
}

/// What a row opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleOpens {
    /// The reveal, not yet shown.
    Reveal,
    /// The detail.
    Detail,
}

/// One style in the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritingStyleEntry {
    /// The style.
    pub id: WritingStyleId,
    /// The person's name for it.
    pub name: String,
    /// What it was learned from.
    pub facts: StyleFacts,
    /// What the row opens.
    pub opens: StyleOpens,
    /// The row itself.
    pub open: StyleControl,
}

/// One account's slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStyleSlot {
    /// The account's id.
    pub account: String,
    /// Its address, which the person reads.
    pub address: String,
    /// The style it drafts in, or none.
    pub selected: Option<WritingStyleId>,
    /// The picker.
    pub picker: StyleControl,
}

/// Why Learn cannot be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnBlock {
    /// No account is used for mail.
    NoMailAccount,
    /// There is nowhere to send, the gate refuses the endpoint as declared, or the Allodia
    /// sign-in needs renewing.
    CannotSend {
        /// Why.
        failure: AiFailure,
        /// Its remedy, held while a change to the library runs.
        remedy: Option<RemedyControl>,
    },
}

/// The run the page draws in Learn's place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRun {
    /// Under way.
    Learning {
        /// Requests answered.
        done: u32,
        /// Requests the run makes; zero until it knows.
        total: u32,
        /// Stop.
        stop: StyleControl,
    },
    /// It failed after its host was closed; Learn opens the flow on the failure.
    Failed {
        /// Why.
        failure: AiFailure,
        /// Its remedy, held while a change to the library runs.
        remedy: Option<RemedyControl>,
    },
}

/// Settings → Writing style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritingStylePageView {
    /// The library, in the person's order.
    pub styles: Vec<WritingStyleEntry>,
    /// One slot per account used for mail, once the library holds a style.
    pub accounts: Vec<AccountStyleSlot>,
    /// Learn.
    pub learn: StyleControl,
    /// Why Learn cannot be pressed, said on the page.
    pub learn_blocked: Option<LearnBlock>,
    /// The run, drawn in Learn's place.
    pub run: Option<PageRun>,
    /// What the last change to the library came to, until the next starts.
    pub outcome: Option<WritingStyleOutcome>,
}

/// The description stored with a style, or an empty one when it cannot be read.
pub(crate) fn guide_of(json: &str) -> StyleGuide {
    serde_json::from_str(json).unwrap_or_default()
}

impl<P: Provider> App<P> {
    /// Settings → Writing style, or `None` while the assistant is not offered. Read from memory,
    /// so a UI thread may ask it.
    #[must_use]
    pub fn writing_style_page(&self) -> Option<WritingStylePageView> {
        if !self.ai_offer() {
            return None;
        }
        let (work, outcome) = self.style_work_view();
        let busy = !work.is_empty();
        let runs = |kind: WritingStyleChangeKind, account: Option<&str>| {
            work.iter().any(|running| {
                running.kind == kind
                    && account.is_none_or(|a| running.account.as_deref() == Some(a))
            })
        };
        let styles: Vec<WritingStyleEntry> = self
            .writing_styles()
            .into_iter()
            .map(|(id, style)| WritingStyleEntry {
                facts: reveal::facts(&style, &guide_of(&style.guide_json)),
                name: style.name,
                opens: if style.revealed {
                    StyleOpens::Detail
                } else {
                    StyleOpens::Reveal
                },
                open: StyleControl::of(busy, false),
                id,
            })
            .collect();
        let senders = self.accounts.senders();
        let accounts = if styles.is_empty() {
            Vec::new()
        } else {
            senders
                .iter()
                .map(|(id, address)| AccountStyleSlot {
                    account: id.as_str().to_owned(),
                    address: address.clone(),
                    selected: self.writing_style_slot(id.as_str()),
                    picker: StyleControl::of(
                        busy,
                        runs(WritingStyleChangeKind::Assign, Some(id.as_str())),
                    ),
                })
                .collect()
        };
        let learn_blocked = if senders.is_empty() {
            Some(LearnBlock::NoMailAccount)
        } else {
            self.ai_route().err().map(|failure| LearnBlock::CannotSend {
                failure,
                remedy: failure.remedy_control(!busy),
            })
        };
        let run = self.learning_on_page(busy);
        let held = busy || learn_blocked.is_some() || matches!(run, Some(PageRun::Learning { .. }));
        Some(WritingStylePageView {
            styles,
            accounts,
            learn: StyleControl::of(held, false),
            learn_blocked,
            run,
            outcome,
        })
    }
}

#[cfg(test)]
mod tests_flow;
#[cfg(test)]
mod tests_page;
#[cfg(test)]
mod tests_reveal_close;
#[cfg(test)]
mod tests_support;
#[cfg(test)]
mod tests_trail;
