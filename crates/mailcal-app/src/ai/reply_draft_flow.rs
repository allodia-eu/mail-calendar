//! What changes a composition's drafted reply: Draft and Stop, the editor's reports, the answers
//! to the two questions, the person's ticks, and the mark a drafted reply carries when it is sent
//! (`docs/ai.md` rules 20 to 25). Each returns at once; a draft itself is made as queued work.

use engine_api::{AccountId, Draft, Provider};
use mailcal_account::drafted_reply_keyword;
use mailcal_ai::{TaskKind, tasks_done};
use mailcal_composer::ReplyDraftReport;

use super::{
    AiFailure, AiQueued, AiWork, AiWorkKey,
    reply_draft::{HeldDraft, Phase, asks_again},
};
use crate::{App, ComposeKind, CompositionId, Surface};

/// What a press of Draft asks for.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplyDraftStart {
    /// The account the reply is sent from, when the composer's From names one.
    pub from: Option<AccountId>,
    /// The instruction as typed; the core trims it.
    pub instruction: String,
    /// The catalog locale the app is shown in, which the summary and the checklist are written in.
    pub language: String,
    /// Whether the person was asked and answered that the draft replaces their text.
    pub replace: bool,
}

impl std::fmt::Debug for ReplyDraftStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplyDraftStart")
            .field("replace", &self.replace)
            .finish_non_exhaustive()
    }
}

/// A draft, as the work that made it hands it over.
pub(super) struct Made {
    pub(super) text: String,
    pub(super) summary: String,
    pub(super) tasks: Vec<mailcal_ai::DraftTask>,
}

impl<P: Provider> App<P> {
    /// Starts drafting `composition`'s reply, and answers how it was queued; `None` when the core
    /// refuses the press: the composition is not a reply, it cannot draft now, a draft is already
    /// being made, or the person's text would be replaced without being asked (rule 22). On
    /// [`AiQueued::Run`] the caller runs [`App::run_ai_work`] off its own thread.
    pub fn draft_reply(
        &self,
        composition: &CompositionId,
        start: ReplyDraftStart,
    ) -> Option<AiQueued> {
        if !self.ai_offer() {
            return None;
        }
        let account = self.reply_of(composition, |reply| reply.account.clone())?;
        let sender = start.from.clone().unwrap_or(account);
        if let Err(failure) = self.reply_draft_availability(&sender) {
            log::info!("ai: a draft was asked for that cannot be made ({failure:?})");
            return None;
        }
        let instruction = start.instruction.trim().to_owned();
        let serial = self.reply_of(composition, |reply| {
            let replying = matches!(reply.kind, ComposeKind::Reply | ComposeKind::ReplyAll);
            let held = reply.held;
            let busy = held.busy() || matches!(held.phase, Phase::Gone);
            if !replying || busy || (held.asks_before_replacing() && !start.replace) {
                return None;
            }
            held.serial += 1;
            held.phase = Phase::Drafting {
                serial: held.serial,
                edits: held.report.as_ref().map(|report| report.edits),
            };
            held.instruction.clone_from(&instruction);
            Some(held.serial)
        })??;
        let key = AiWorkKey::Drafting(composition.clone());
        let held = composition.clone();
        let work = AiWork::new(key, move |app: &Self, run| {
            let made = app.make_reply_draft(&held, &sender, &instruction, &start.language, run);
            if let Some(made) = made {
                app.settle_reply_draft(&held, serial, made);
            }
        });
        let queued = self.queue_ai_work(work);
        if queued == AiQueued::NoHost {
            self.settle_reply_draft(composition, serial, Err(AiFailure::NowhereToSend));
        }
        Some(queued)
    }

    /// Records what the draft under `serial` came to, unless it was stopped, replaced or its
    /// composition has gone meanwhile.
    pub(super) fn settle_reply_draft(
        &self,
        composition: &CompositionId,
        serial: u64,
        made: Result<Made, AiFailure>,
    ) {
        let settled = self.reply_of(composition, |reply| {
            let held = reply.held;
            let Phase::Drafting {
                serial: running,
                edits,
            } = held.phase
            else {
                return false;
            };
            if running != serial {
                return false;
            }
            held.phase = match made {
                Ok(made) => {
                    let draft = HeldDraft {
                        serial,
                        text: made.text,
                        summary: made.summary,
                        tasks: made.tasks,
                        ticked: std::collections::BTreeSet::new(),
                        went_in: false,
                        files_mark: None,
                    };
                    if asks_again(held.report.as_ref(), edits) {
                        Phase::Arrived(draft)
                    } else {
                        held.draft = Some(draft);
                        Phase::Idle
                    }
                }
                Err(AiFailure::MessageGone) => Phase::Gone,
                Err(failure) => Phase::Failed(failure),
            };
            true
        });
        if settled == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// Stops the draft being made, the request in flight included, or drops one that arrived and
    /// waits for the replace question or for the editor. Stop is not a failure: the control is as
    /// it was.
    pub fn stop_reply_draft(&self, composition: &CompositionId) {
        let stopped = self.reply_of(composition, |reply| {
            let held = reply.held;
            let busy = held.busy();
            if busy {
                held.phase = Phase::Idle;
                if held.draft.as_ref().is_some_and(|draft| !draft.went_in) {
                    held.draft = None;
                }
            }
            busy
        });
        self.cancel_ai_work(&AiWorkKey::Drafting(composition.clone()));
        if stopped == Some(true) {
            log::info!("ai: a draft was stopped");
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// The answer to a draft that arrived after the person wrote meanwhile: `replace` hands it to
    /// the editor, otherwise their text stays and the draft is dropped.
    pub fn answer_reply_draft_arrived(&self, composition: &CompositionId, replace: bool) {
        let answered = self.reply_of(composition, |reply| {
            let Phase::Arrived(draft) = std::mem::take(&mut reply.held.phase) else {
                return false;
            };
            if replace {
                reply.held.draft = Some(draft);
            }
            true
        });
        if answered == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// Takes what the editor says about the reply (`ReplyDraftReport`). `files_mark` is where the
    /// composition's file list stands now, kept when the draft first goes in.
    pub fn take_reply_draft_report(
        &self,
        composition: &CompositionId,
        report: ReplyDraftReport,
        files_mark: u64,
    ) {
        let changed = self.reply_of(composition, |reply| {
            let held = reply.held;
            if let Some(draft) = held.draft.as_mut()
                && draft.serial == report.serial
                && report.inserted
            {
                held.drafted = true;
                draft.went_in = true;
                draft.files_mark.get_or_insert(files_mark);
            }
            let changed = held.report.as_ref() != Some(&report);
            held.report = Some(report);
            changed
        });
        if changed == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// Where `composition`'s file list is counted from while a file can change its record: a draft
    /// with a file to attach is in the reply. `None` otherwise, when no file changes the record.
    #[must_use]
    pub fn reply_draft_files_mark(&self, composition: &CompositionId) -> Option<u64> {
        self.reply_of(composition, |reply| {
            let draft = reply.held.inserted()?;
            let attaches = draft.tasks.iter().any(|task| task.kind == TaskKind::Attach);
            attaches.then_some(draft.files_mark).flatten()
        })
        .flatten()
    }

    /// `composition`'s file list changed, from `before` files added since
    /// [`App::reply_draft_files_mark`] to `after`. Signals [`Surface::ReplyDraft`] when that
    /// changes what the record says: an item to attach done or open, or the question Send asks.
    pub fn reply_draft_files_changed(
        &self,
        composition: &CompositionId,
        before: usize,
        after: usize,
    ) {
        let changed = self.reply_of(composition, |reply| {
            let held = &*reply.held;
            let Some(draft) = held.inserted() else {
                return false;
            };
            let reads = |added: usize| {
                (
                    tasks_done(&draft.tasks, &held.progress(draft, added)),
                    held.send_question(&|_| added),
                )
            };
            reads(before) != reads(after)
        });
        if changed == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// The person's tick on item `index` of the draft in the reply. A gap follows the reply and is
    /// never ticked by hand.
    pub fn tick_reply_draft_item(&self, composition: &CompositionId, index: usize, done: bool) {
        let ticked = self.reply_of(composition, |reply| {
            let Some(draft) = reply.held.draft.as_mut() else {
                return false;
            };
            if draft
                .tasks
                .get(index)
                .is_none_or(|task| task.kind == TaskKind::FillIn)
            {
                return false;
            }
            if done {
                draft.ticked.insert(index)
            } else {
                draft.ticked.remove(&index)
            }
        });
        if ticked == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// The open items Send asks about for `composition` now (rule 24), while the question is due:
    /// what its record says, and what the submit path refuses by, so the two cannot disagree.
    #[must_use]
    pub fn reply_draft_send_question(
        &self,
        composition: &CompositionId,
        files_added: &dyn Fn(u64) -> usize,
    ) -> Option<u32> {
        self.reply_of(composition, |reply| reply.held.send_question(files_added))
            .flatten()
    }

    /// Send asked about open items and the person answered, either way: it is not asked again in
    /// this composition. Applied before this returns, so a submit right after it sends.
    pub fn answer_reply_draft_send_question(&self, composition: &CompositionId) {
        let answered = self.reply_of(composition, |reply| {
            !std::mem::replace(&mut reply.held.send_asked, true)
        });
        if answered == Some(true) {
            self.observer.surface_changed(Surface::ReplyDraft);
        }
    }

    /// Whether a draft ever went into `composition` (rule 25).
    #[must_use]
    pub fn composition_drafted(&self, composition: &CompositionId) -> bool {
        self.reply_of(composition, |reply| reply.held.drafted)
            .unwrap_or(false)
    }

    /// `draft`, about to be sent from `account` for `composition`, marked as a drafted reply when
    /// a draft ever went in: its `Message-ID` noted in this device's log, and the keyword asked
    /// for on the Sent copy. Neither can fail or delay the send.
    pub(crate) fn mark_drafted_reply(
        &self,
        composition: Option<&CompositionId>,
        account: &AccountId,
        draft: Draft,
    ) -> Draft {
        if !composition.is_some_and(|composition| self.composition_drafted(composition)) {
            return draft;
        }
        self.note_drafted_reply(account.as_str(), draft.message_id.clone());
        draft.with_sent_copy_keyword(drafted_reply_keyword())
    }
}
