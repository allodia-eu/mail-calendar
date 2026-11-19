//! Settings → Writing style as the core decides it (`docs/ai.md` rules 8 to 10). A client draws
//! this and words it; every control is a `ControlState`.

use mailcal_app::ai as core;

use crate::{AiFailure, AiRemedy, ControlState, WritingStyleFacts};

/// Settings → Writing style (pulled after `Surface::WritingStyle` and `Surface::Settings`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStylePage {
    /// The library, in the person's order.
    pub styles: Vec<WritingStyleEntry>,
    /// A slot picker per account used for mail, once the library holds a style; empty before.
    pub accounts: Vec<WritingStyleSlot>,
    /// Learn, which opens the flow (`StyleLearningIntent::Open`).
    pub learn: ControlState,
    /// Why Learn cannot be pressed, said on the page with its one remedy.
    pub learn_blocked: Option<WritingStyleLearnBlock>,
    /// The run, drawn in Learn's place while it goes or after it failed with its host closed.
    pub run: Option<WritingStyleRun>,
    /// What the last library change came to, until the next starts.
    pub outcome: Option<WritingStyleOutcome>,
}

/// One style in the library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleEntry {
    pub id: String,
    pub name: String,
    pub facts: WritingStyleFacts,
    /// What the row opens: the reveal once, the detail ever after.
    pub opens: WritingStyleOpens,
    pub open: ControlState,
}

/// What a library row opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleOpens {
    /// `MailcalApp::writing_style_reveal`.
    Reveal,
    /// `MailcalApp::writing_style_detail`.
    Detail,
}

/// One account's slot: a style from the library, or none.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleSlot {
    /// The account's id, which `WritingStyleIntent::Assign` names it by.
    pub account_id: String,
    /// The account's address, its label.
    pub email: String,
    pub selected: Option<String>,
    pub picker: ControlState,
}

/// Why Learn cannot be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleLearnBlock {
    /// No account is used for mail.
    NoMailAccount,
    /// Nowhere to send, the gate refusing the endpoint as declared, or the Allodia sign-in to
    /// renew.
    CannotSend {
        failure: AiFailure,
        remedy: Option<AiRemedy>,
    },
}

/// The run as the page draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleRun {
    /// Under way: progress and Stop (`StyleLearningIntent::Stop`).
    Learning {
        done: u32,
        /// Zero until the run knows; a client then draws no fraction.
        total: u32,
        stop: ControlState,
    },
    /// It failed after its host was closed. Learn reopens the flow on the failure.
    Failed {
        failure: AiFailure,
        remedy: Option<AiRemedy>,
    },
}

/// Which change to the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleChangeKind {
    Rename,
    Notes,
    Assign,
    Forget,
    SaveReveal,
    CloseReveal,
}

/// Why a change to the library was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleRefusal {
    /// The name is empty or only spaces.
    BlankName,
    /// The style was forgotten meanwhile.
    Gone,
}

/// What a change to the library came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WritingStyleResult {
    /// Made, and written.
    Done,
    /// Not made, for this reason: the surface that sent it stays and says why.
    Refused { reason: WritingStyleRefusal },
    /// Made, and not written, so it holds until the app closes; the failed-save notice says so.
    NotWritten,
}

/// The last change to the library and what it came to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WritingStyleOutcome {
    /// Grows with each change, so a client acts on each outcome once.
    pub serial: u64,
    pub kind: WritingStyleChangeKind,
    /// The style it changed; `None` for an account's slot.
    pub style: Option<String>,
    /// The account whose slot it set; `None` for a change to a style.
    pub account_id: Option<String>,
    pub result: WritingStyleResult,
}

pub(super) fn control(control: core::StyleControl) -> ControlState {
    ControlState {
        enabled: control.enabled,
        running: control.running,
    }
}

pub(super) fn failed(failure: core::AiFailure) -> (AiFailure, Option<AiRemedy>) {
    (failure.into(), failure.remedy().map(Into::into))
}

fn kind(kind: core::WritingStyleChangeKind) -> WritingStyleChangeKind {
    match kind {
        core::WritingStyleChangeKind::Rename => WritingStyleChangeKind::Rename,
        core::WritingStyleChangeKind::Notes => WritingStyleChangeKind::Notes,
        core::WritingStyleChangeKind::Assign => WritingStyleChangeKind::Assign,
        core::WritingStyleChangeKind::Forget => WritingStyleChangeKind::Forget,
        core::WritingStyleChangeKind::SaveReveal => WritingStyleChangeKind::SaveReveal,
        core::WritingStyleChangeKind::CloseReveal => WritingStyleChangeKind::CloseReveal,
    }
}

impl From<core::WritingStyleRefusal> for WritingStyleRefusal {
    fn from(refusal: core::WritingStyleRefusal) -> Self {
        match refusal {
            core::WritingStyleRefusal::BlankName => Self::BlankName,
            core::WritingStyleRefusal::Gone => Self::Gone,
        }
    }
}

/// The page's record.
pub(crate) fn page(view: core::WritingStylePageView) -> WritingStylePage {
    WritingStylePage {
        styles: view
            .styles
            .into_iter()
            .map(|entry| WritingStyleEntry {
                id: entry.id.as_str().to_owned(),
                name: entry.name,
                facts: entry.facts.into(),
                opens: match entry.opens {
                    core::StyleOpens::Reveal => WritingStyleOpens::Reveal,
                    core::StyleOpens::Detail => WritingStyleOpens::Detail,
                },
                open: control(entry.open),
            })
            .collect(),
        accounts: view
            .accounts
            .into_iter()
            .map(|slot| WritingStyleSlot {
                account_id: slot.account,
                email: slot.address,
                selected: slot.selected.map(|id| id.as_str().to_owned()),
                picker: control(slot.picker),
            })
            .collect(),
        learn: control(view.learn),
        learn_blocked: view.learn_blocked.map(|block| match block {
            core::LearnBlock::NoMailAccount => WritingStyleLearnBlock::NoMailAccount,
            core::LearnBlock::CannotSend(failure) => {
                let (failure, remedy) = failed(failure);
                WritingStyleLearnBlock::CannotSend { failure, remedy }
            }
        }),
        run: view.run.map(|run| match run {
            core::PageRun::Learning { done, total, stop } => WritingStyleRun::Learning {
                done,
                total,
                stop: control(stop),
            },
            core::PageRun::Failed(failure) => {
                let (failure, remedy) = failed(failure);
                WritingStyleRun::Failed { failure, remedy }
            }
        }),
        outcome: view.outcome.map(|outcome| WritingStyleOutcome {
            serial: outcome.serial,
            kind: kind(outcome.work.kind),
            style: outcome.work.style.map(|id| id.as_str().to_owned()),
            account_id: outcome.work.account,
            result: match outcome.result {
                core::WritingStyleResult::Done => WritingStyleResult::Done,
                core::WritingStyleResult::Refused { reason } => WritingStyleResult::Refused {
                    reason: reason.into(),
                },
                core::WritingStyleResult::NotWritten => WritingStyleResult::NotWritten,
            },
        }),
    }
}
