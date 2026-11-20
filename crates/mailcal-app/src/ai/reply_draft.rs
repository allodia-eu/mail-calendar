//! Draft a reply, as the composer draws it (`docs/ai.md` rules 17 to 24): whether it is offered,
//! what it says, which control is held and why, the card a draft brings and the question Send
//! asks. Held on the composition; read with [`App::composer_reply_draft`] and changed by the
//! functions in `reply_draft_flow`.
//!
//! **Privacy.** A draft, its summary and its items are the person's mail; nothing here logs them.

use std::{collections::BTreeSet, fmt};

use engine_api::{AccountId, Provider};
use mailcal_ai::{ChecklistProgress, DraftTask, TaskKind, tasks_done, tasks_open};
use mailcal_composer::{LeadText, ReplyDraftReport};

use super::{AiFailure, AiRemedy, RemedyControl};
use crate::{App, ComposeKind, CompositionId};

/// What the core holds of one composition's drafted reply.
#[derive(Default)]
pub(crate) struct ReplyDraftHeld {
    /// Numbers each start; grows only.
    pub(super) serial: u64,
    pub(super) phase: Phase,
    /// The draft last put in the reply, or handed to the editor to put there.
    pub(super) draft: Option<HeldDraft>,
    /// What the editor last reported.
    pub(super) report: Option<ReplyDraftReport>,
    /// The instruction the last draft was asked with, which the input opens holding.
    pub(super) instruction: String,
    /// Whether a draft ever went into the reply. Never cleared: an undone or rewritten draft may
    /// still have left its words (rule 25).
    pub(super) drafted: bool,
    /// Whether Send asked about open items, whatever the answer.
    pub(super) send_asked: bool,
    /// Whether the composer opened holding no body of its own, only what the core seeds: the
    /// quoted original and the signature.
    opened_blank: bool,
}

impl ReplyDraftHeld {
    /// What a composition starts with. `opened_blank` is false when its composer opened holding a
    /// body of its own: a stored draft's, or a prefill's text. `drafted` when what it opened on
    /// carries the drafted-reply mark (rule 25); the card it had is not stored, so none is drawn.
    pub(crate) fn opening(opened_blank: bool, drafted: bool) -> Self {
        Self {
            drafted,
            opened_blank,
            ..Self::default()
        }
    }

    /// Whether a press of Draft must ask first: while the person's own text is above the
    /// signature and the quote, by what the editor last said, and before it has said, while the
    /// composer opened holding a body of its own.
    pub(super) fn asks_before_replacing(&self) -> bool {
        self.report.as_ref().map_or(!self.opened_blank, |report| {
            report.lead == LeadText::Written
        })
    }
}

#[derive(Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    /// A draft is being made, under `serial`. `edits` is the editor's count when it was asked
    /// for, so a person who wrote meanwhile is asked again.
    Drafting {
        serial: u64,
        edits: Option<u64>,
    },
    /// A draft arrived after the person wrote above the signature and the quote meanwhile; it
    /// waits for their answer.
    Arrived(HeldDraft),
    Failed(AiFailure),
    /// The message answered is no longer on this device.
    Gone,
}

/// One draft as the core keeps it.
pub(super) struct HeldDraft {
    pub(super) serial: u64,
    pub(super) text: String,
    pub(super) summary: String,
    pub(super) tasks: Vec<DraftTask>,
    pub(super) ticked: BTreeSet<usize>,
    /// Whether the editor reported it in the reply at least once.
    pub(super) went_in: bool,
    /// Where the composition's file list stood when it went in, as the list numbers its files.
    pub(super) files_mark: Option<u64>,
}

impl HeldDraft {
    /// Its gaps, in the order the editor is given them.
    pub(super) fn gaps(&self) -> Vec<&str> {
        self.tasks
            .iter()
            .filter(|task| task.kind == TaskKind::FillIn)
            .map(|task| task.text.as_str())
            .collect()
    }
}

/// One of the instruction's suggestions (rule 19). The client words it, and the words fill the
/// line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyDraftSuggestion {
    /// Yes.
    Yes,
    /// No.
    No,
    /// More information is needed.
    MoreInformation,
    /// A holding reply: an answer follows.
    Holding,
}

/// What the suggestions are, in the order they are drawn.
pub const REPLY_DRAFT_SUGGESTIONS: [ReplyDraftSuggestion; 4] = [
    ReplyDraftSuggestion::Yes,
    ReplyDraftSuggestion::No,
    ReplyDraftSuggestion::MoreInformation,
    ReplyDraftSuggestion::Holding,
];

/// Whether a control can be acted on, and whether work it started runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyDraftControl {
    /// It can be acted on now.
    pub enabled: bool,
    /// Work it started runs; never enabled meanwhile.
    pub running: bool,
}

const HELD: ReplyDraftControl = ReplyDraftControl {
    enabled: false,
    running: false,
};

/// One item of a draft's checklist.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplyDraftItem {
    /// What kind of thing it is.
    pub kind: TaskKind,
    /// The gap exactly as the reply carries it, or the task in the interface's language.
    pub text: String,
    /// Whether it is done ([`mailcal_ai::tasks_done`]).
    pub done: bool,
    /// The person's tick; `None` for a gap, whose `done` follows the reply and is drawn as a
    /// state, not as a control.
    pub tick: Option<ReplyDraftControl>,
}

impl fmt::Debug for ReplyDraftItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplyDraftItem")
            .field("kind", &self.kind)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

/// Where Draft a reply stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyDraftState {
    /// It cannot draft now, for `failure`.
    Unavailable {
        /// Why.
        failure: AiFailure,
        /// What it offers.
        remedy: Option<RemedyControl>,
    },
    /// It can draft.
    Ready,
    /// A draft is being made, or waits for the replace question: a note with Stop and without a
    /// spinner, since the action row's control shows the work.
    Drafting,
    /// A draft is in the reply, with what the message asks and what is left to do.
    Drafted {
        /// What the message asks, in a sentence or two; empty when none came.
        summary: String,
        /// What is left to do, at most six of the model's items after the gaps.
        checklist: Vec<ReplyDraftItem>,
    },
    /// The last draft did not come.
    Failed {
        /// Why.
        failure: AiFailure,
        /// What it offers: Try again is held with Draft.
        remedy: Option<RemedyControl>,
    },
}

/// Why Draft cannot be pressed, and its remedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyDraftHold {
    /// Why.
    pub failure: AiFailure,
    /// What it offers, each opening what it names.
    pub remedy: Option<RemedyControl>,
}

impl ReplyDraftHold {
    const fn of(failure: AiFailure) -> Self {
        Self {
            failure,
            remedy: failure.remedy_control(true),
        }
    }
}

/// The script that hands a draft to the editor, run once per `serial` in each editor.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplyDraftScript {
    /// The draft it carries.
    pub serial: u64,
    /// What the client runs in the editor.
    pub script: String,
}

impl fmt::Debug for ReplyDraftScript {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplyDraftScript")
            .field("serial", &self.serial)
            .finish_non_exhaustive()
    }
}

/// Draft a reply, as a composer draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraftView {
    /// Where it stands.
    pub state: ReplyDraftState,
    /// Draft a reply in the action row: running while a draft is made, the one control that
    /// shows the work; held only when the message answered is gone.
    pub control: ReplyDraftControl,
    /// Draft, in the instruction's input.
    pub draft: ReplyDraftControl,
    /// Stop, while a draft is made. Never running: Stop ends the work at once.
    pub stop: ReplyDraftControl,
    /// Why Draft cannot be pressed, said in the input in place of the field.
    pub held_by: Option<ReplyDraftHold>,
    /// What the instruction's line offers under it (rule 19).
    pub suggestions: Vec<ReplyDraftSuggestion>,
    /// The instruction the last draft was asked with.
    pub instruction: String,
    /// Whether pressing Draft must first ask whether the draft replaces what is above the
    /// signature and the quote (rule 22).
    pub ask_before_replacing: bool,
    /// Whether a draft that arrived waits for that question, the person having written meanwhile.
    pub replace_arrived: bool,
    /// The draft for the editor, run once per serial in each editor.
    pub editor_script: Option<ReplyDraftScript>,
    /// The open items Send asks about, while it is due (rule 24).
    pub send_question: Option<u32>,
}

/// Whether a draft arriving is asked about again: the person wrote above the signature and the
/// quote since they pressed Draft, and it is theirs now.
pub(super) fn asks_again(report: Option<&ReplyDraftReport>, edits_at_press: Option<u64>) -> bool {
    report.is_some_and(|report| {
        report.lead == LeadText::Written && Some(report.edits) != edits_at_press
    })
}

impl ReplyDraftHeld {
    /// The checklist's progress for `draft`, with `files_added` counting the files added since
    /// it went in.
    pub(super) fn progress(&self, draft: &HeldDraft, files_added: usize) -> ChecklistProgress {
        let gaps = draft.gaps();
        let gaps_left = match self.report.as_ref() {
            Some(report) if report.serial == draft.serial => report
                .left
                .iter()
                .filter_map(|at| gaps.get(usize::try_from(*at).ok()?))
                .map(|gap| (*gap).to_owned())
                .collect(),
            // Until the editor has said, every gap is open.
            _ => gaps.iter().map(|gap| (*gap).to_owned()).collect(),
        };
        ChecklistProgress {
            gaps_left,
            files_added,
            ticked: draft.ticked.clone(),
        }
    }

    /// Whether a draft is being made, waits for the replace question, or was handed to the editor
    /// and has not gone in yet: drawn as drafting, with Stop.
    pub(super) fn busy(&self) -> bool {
        matches!(self.phase, Phase::Drafting { .. } | Phase::Arrived(_))
            || self.draft.as_ref().is_some_and(|draft| !draft.went_in)
    }

    /// The draft in the reply now, by the editor's word.
    pub(super) fn inserted(&self) -> Option<&HeldDraft> {
        let draft = self.draft.as_ref()?;
        let report = self.report.as_ref()?;
        (report.serial == draft.serial && report.inserted).then_some(draft)
    }

    /// The open items Send asks about, while it is due: a draft is in the reply, Send has not
    /// asked in this composition, and the checklist has an item open.
    pub(super) fn send_question(&self, files_added: &dyn Fn(u64) -> usize) -> Option<u32> {
        if self.send_asked {
            return None;
        }
        let draft = self.inserted()?;
        let added = draft.files_mark.map_or(0, files_added);
        let open = tasks_open(&draft.tasks, &self.progress(draft, added));
        (open > 0).then(|| u32::try_from(open).unwrap_or(u32::MAX))
    }
}

impl<P: Provider> App<P> {
    /// Draft a reply for `composition`'s composer, sending from `from` (its From, or the account
    /// it opened on). `None` while the assistant is not offered or the composition is not a reply
    /// (rule 17). `files_added` answers how many files were added since the given mark of the
    /// composition's file list. Read from memory, so a UI thread may ask it.
    #[must_use]
    pub fn composer_reply_draft(
        &self,
        composition: &CompositionId,
        from: Option<&AccountId>,
        files_added: &dyn Fn(u64) -> usize,
    ) -> Option<ReplyDraftView> {
        if !self.ai_offer() {
            return None;
        }
        let (account, view) = self.reply_of(composition, |reply| {
            matches!(reply.kind, ComposeKind::Reply | ComposeKind::ReplyAll)
                .then(|| (reply.account.clone(), held_view(reply.held, files_added)))
        })??;
        let account = from.cloned().unwrap_or(account);
        Some(view.finish(self.reply_draft_availability(&account).err()))
    }

    /// Why `account` cannot draft now, or nothing: a style first, then where to send.
    pub(super) fn reply_draft_availability(&self, account: &AccountId) -> Result<(), AiFailure> {
        if self.writing_styles().is_empty() {
            return Err(AiFailure::NoStyle);
        }
        if self.writing_style_slot(account.as_str()).is_none() {
            return Err(AiFailure::NoStyleForAccount);
        }
        self.ai_route().map(drop)
    }
}

/// What the held state says before availability is known.
struct HeldView {
    state: Option<ReplyDraftState>,
    drafting: bool,
    instruction: String,
    ask_before_replacing: bool,
    replace_arrived: bool,
    editor_script: Option<ReplyDraftScript>,
    send_question: Option<u32>,
}

fn held_view(held: &ReplyDraftHeld, files_added: &dyn Fn(u64) -> usize) -> HeldView {
    let drafted = held.inserted().map(|draft| {
        let added = draft.files_mark.map_or(0, files_added);
        let done = tasks_done(&draft.tasks, &held.progress(draft, added));
        ReplyDraftState::Drafted {
            summary: draft.summary.clone(),
            checklist: draft
                .tasks
                .iter()
                .zip(done)
                .map(|(task, done)| ReplyDraftItem {
                    kind: task.kind,
                    text: task.text.clone(),
                    done,
                    tick: (task.kind != TaskKind::FillIn).then_some(ReplyDraftControl {
                        enabled: true,
                        running: false,
                    }),
                })
                .collect(),
        }
    });
    // Try again is held with Draft, which `HeldView::finish` decides.
    let failed = |failure: AiFailure| ReplyDraftState::Failed {
        failure,
        remedy: failure.remedy_control(true),
    };
    let state = match &held.phase {
        _ if held.busy() => Some(ReplyDraftState::Drafting),
        Phase::Failed(failure) => Some(failed(*failure)),
        Phase::Gone => Some(ReplyDraftState::Unavailable {
            failure: AiFailure::MessageGone,
            remedy: None,
        }),
        Phase::Idle | Phase::Drafting { .. } | Phase::Arrived(_) => drafted,
    };
    HeldView {
        state,
        drafting: held.busy(),
        instruction: held.instruction.clone(),
        ask_before_replacing: held.asks_before_replacing(),
        replace_arrived: matches!(held.phase, Phase::Arrived(_)),
        editor_script: held.draft.as_ref().map(editor_script),
        send_question: held.send_question(files_added),
    }
}

impl HeldView {
    fn finish(self, unavailable: Option<AiFailure>) -> ReplyDraftView {
        let mut state = self.state.unwrap_or_else(|| match unavailable {
            Some(failure) => ReplyDraftState::Unavailable {
                failure,
                remedy: failure.remedy_control(true),
            },
            None => ReplyDraftState::Ready,
        });
        let gone = matches!(
            state,
            ReplyDraftState::Unavailable {
                failure: AiFailure::MessageGone,
                ..
            }
        );
        let held_by = if gone {
            Some(AiFailure::MessageGone)
        } else {
            unavailable
        };
        let draft = ReplyDraftControl {
            enabled: !self.drafting && held_by.is_none(),
            running: false,
        };
        if let ReplyDraftState::Failed {
            remedy:
                Some(RemedyControl {
                    remedy: AiRemedy::TryAgain,
                    enabled,
                }),
            ..
        } = &mut state
        {
            *enabled = draft.enabled;
        }
        ReplyDraftView {
            state,
            control: ReplyDraftControl {
                enabled: !self.drafting && !gone,
                running: self.drafting,
            },
            draft,
            stop: if self.drafting {
                ReplyDraftControl {
                    enabled: true,
                    running: false,
                }
            } else {
                HELD
            },
            held_by: held_by.map(ReplyDraftHold::of),
            suggestions: REPLY_DRAFT_SUGGESTIONS.to_vec(),
            instruction: self.instruction,
            ask_before_replacing: self.ask_before_replacing,
            replace_arrived: self.replace_arrived,
            editor_script: self.editor_script,
            send_question: self.send_question,
        }
    }
}

/// The script that hands `draft` to the editor: to put in the reply as one step of its history
/// until the editor has said it went in, and from then on only to follow, for an editor that
/// opened later on the same composition (a composer moved into a window of its own).
fn editor_script(draft: &HeldDraft) -> ReplyDraftScript {
    let payload = serde_json::json!({
        "serial": draft.serial,
        "text": draft.text,
        "gaps": draft.gaps(),
        "insert": !draft.went_in,
    });
    // JSON allows U+2028 and U+2029 inside a string and older JavaScript did not.
    let json = payload
        .to_string()
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    ReplyDraftScript {
        serial: draft.serial,
        script: format!("window.setComposerDraftText({json});"),
    }
}
