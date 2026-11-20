//! What a composer draws for Draft a reply. The client words each value and decides nothing it
//! says: whether the control is held and why, whether an item is ticked, whether to ask before
//! replacing or at Send.

use std::fmt;

use mailcal_app::ai as core;

use crate::{AiFailure, AiRemedyControl, ControlState};

/// Draft a reply, for one composition, read with `MailcalApp::composer_reply_draft`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ComposerReplyDraft {
    /// Where it stands.
    pub state: ReplyDraftState,
    /// Draft a reply in the action row, beside Signature. Running while a draft is made, and the
    /// one control drawn with a spinner meanwhile; held only when the message answered is gone,
    /// and otherwise it opens the instruction's input.
    pub control: ControlState,
    /// Draft in the instruction's input.
    pub draft: ControlState,
    /// Stop, drawn among the composer's notes while a draft is made. Never running: Stop ends the
    /// work at once.
    pub stop: ControlState,
    /// Why Draft cannot be pressed: said in the input in place of the field, with its remedy;
    /// for `MessageGone`, a line among the composer's notes.
    pub held_by: Option<ReplyDraftHold>,
    /// What the instruction's line offers under it, in this order.
    pub suggestions: Vec<ReplyDraftSuggestion>,
    /// What the input opens holding: the instruction the last draft was asked with.
    pub instruction: String,
    /// Whether a press of Draft asks first whether the draft replaces the person's text above the
    /// signature and the quote; the answer goes in `ReplyDraftIntent::Draft`'s `replace`.
    pub ask_before_replacing: bool,
    /// Whether a draft that arrived waits for that question, the person having written meanwhile;
    /// answered with `ReplyDraftIntent::AnswerArrived`.
    pub replace_arrived: bool,
    /// The draft for the editor: run `script` once per `serial` in each editor.
    pub editor_script: Option<ReplyDraftScript>,
    /// While due, the question Send asks before it sends; `submit_composition` refuses
    /// meanwhile.
    pub send_question: Option<ReplyDraftSendQuestion>,
}

/// Why Draft cannot be pressed, and its one remedy, which opens what it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ReplyDraftHold {
    pub failure: AiFailure,
    pub remedy: Option<AiRemedyControl>,
}

/// Where Draft a reply stands (`docs/ai.md` rule 17).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ReplyDraftState {
    /// It cannot draft now.
    Unavailable {
        failure: AiFailure,
        remedy: Option<AiRemedyControl>,
    },
    /// It can draft.
    Ready,
    /// A draft is being made, or waits for the replace question; drawn among the notes as a line
    /// with Stop and no spinner, since `control` shows the work.
    Drafting,
    /// A draft is in the reply: the card, among the composer's notes.
    Drafted {
        /// What the message asks, in a sentence or two; empty when none came.
        summary: String,
        checklist: Vec<ReplyDraftItem>,
    },
    /// The last draft did not come. Try again opens the instruction's input, and is held with
    /// Draft.
    Failed {
        failure: AiFailure,
        remedy: Option<AiRemedyControl>,
    },
}

/// One item of the card's checklist.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReplyDraftItem {
    /// What `ReplyDraftIntent::Tick` names it by.
    pub index: u32,
    pub kind: ReplyDraftTaskKind,
    /// A gap exactly as the reply carries it, or the task in the interface's language.
    pub text: String,
    pub done: bool,
    /// The person's tick (`ReplyDraftIntent::Tick`). `None` for a gap, whose `done` follows the
    /// reply: a client draws it as a state that cannot be pressed (an open or a done mark beside
    /// the text), never as a disabled checkbox.
    pub tick: Option<ControlState>,
}

impl fmt::Debug for ReplyDraftItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplyDraftItem")
            .field("index", &self.index)
            .field("kind", &self.kind)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

/// What kind of thing an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReplyDraftTaskKind {
    /// A gap in brackets to fill in: done while it is gone from the reply.
    FillIn,
    /// A file to attach: done once files were added after the draft, or ticked.
    Attach,
    /// Anything else to do: done once ticked.
    Do,
}

/// One suggestion under the instruction's line; the client words it and the words fill the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ReplyDraftSuggestion {
    Yes,
    No,
    MoreInformation,
    /// A holding reply: an answer follows.
    Holding,
}

/// The draft for the editor.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct ReplyDraftScript {
    pub serial: u64,
    /// `window.setComposerDraftText(…)`, built by the core.
    pub script: String,
}

impl fmt::Debug for ReplyDraftScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplyDraftScript")
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

/// The question Send asks while the checklist has open items and it was not asked before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct ReplyDraftSendQuestion {
    /// How many items are open.
    pub open: u32,
}

impl From<core::ReplyDraftView> for ComposerReplyDraft {
    fn from(view: core::ReplyDraftView) -> Self {
        Self {
            state: view.state.into(),
            control: control(view.control),
            draft: control(view.draft),
            stop: control(view.stop),
            held_by: view.held_by.map(|hold| ReplyDraftHold {
                failure: hold.failure.into(),
                remedy: hold.remedy.map(Into::into),
            }),
            suggestions: view.suggestions.into_iter().map(Into::into).collect(),
            instruction: view.instruction,
            ask_before_replacing: view.ask_before_replacing,
            replace_arrived: view.replace_arrived,
            editor_script: view.editor_script.map(|script| ReplyDraftScript {
                serial: script.serial,
                script: script.script,
            }),
            send_question: view
                .send_question
                .map(|open| ReplyDraftSendQuestion { open }),
        }
    }
}

const fn control(control: core::ReplyDraftControl) -> ControlState {
    ControlState {
        enabled: control.enabled,
        running: control.running,
    }
}

impl From<core::ReplyDraftState> for ReplyDraftState {
    fn from(state: core::ReplyDraftState) -> Self {
        match state {
            core::ReplyDraftState::Unavailable { failure, remedy } => Self::Unavailable {
                failure: failure.into(),
                remedy: remedy.map(Into::into),
            },
            core::ReplyDraftState::Ready => Self::Ready,
            core::ReplyDraftState::Drafting => Self::Drafting,
            core::ReplyDraftState::Drafted { summary, checklist } => Self::Drafted {
                summary,
                checklist: checklist
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| ReplyDraftItem {
                        index: u32::try_from(index).unwrap_or(u32::MAX),
                        kind: match item.kind {
                            mailcal_ai::TaskKind::FillIn => ReplyDraftTaskKind::FillIn,
                            mailcal_ai::TaskKind::Attach => ReplyDraftTaskKind::Attach,
                            mailcal_ai::TaskKind::Do => ReplyDraftTaskKind::Do,
                        },
                        text: item.text,
                        done: item.done,
                        tick: item.tick.map(control),
                    })
                    .collect(),
            },
            core::ReplyDraftState::Failed { failure, remedy } => Self::Failed {
                failure: failure.into(),
                remedy: remedy.map(Into::into),
            },
        }
    }
}

impl From<core::ReplyDraftSuggestion> for ReplyDraftSuggestion {
    fn from(suggestion: core::ReplyDraftSuggestion) -> Self {
        match suggestion {
            core::ReplyDraftSuggestion::Yes => Self::Yes,
            core::ReplyDraftSuggestion::No => Self::No,
            core::ReplyDraftSuggestion::MoreInformation => Self::MoreInformation,
            core::ReplyDraftSuggestion::Holding => Self::Holding,
        }
    }
}
