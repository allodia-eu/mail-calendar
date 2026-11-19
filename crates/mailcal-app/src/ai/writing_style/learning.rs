//! The learning flow as the core holds it (`docs/ai.md` rules 11 to 14): the record a host draws,
//! the intents that move it, the read the review runs and the run the consent queues.
//!
//! The read is an engine read and sends nothing, so it runs on the host's runtime beside the
//! queue; the run sends, so it is queued work under [`AiWorkKey::Learning`], one at a time.

use engine_api::{AccountId, Provider};
use mailcal_account::{StoredWritingStyle, WritingStyleId};
use mailcal_ai::{
    LearnOptions, LearnProgress,
    corpus::{self, CorpusOptions},
    learn_style,
};

use super::{
    PageRun, StyleControl, WritingStyleChange,
    flow::{Flow, Flows, LearningIntent, LearningTrailStage, Review, Reviewed, Stage, Start},
    range::{LearnRange, LearnRanges, SentReach, learn_ranges, today},
};
use crate::{
    App, Surface,
    ai::{AiFailure, AiRoute, AiRun, AiWork, AiWorkKey, RemedyControl},
};

/// One account the flow can learn from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnAccount {
    /// The account's id, which `SelectAccount` names it by.
    pub id: String,
    /// Its address, which the person reads.
    pub address: String,
}

/// The stage on screen, with what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningStage {
    /// Which account to learn from; only with more than one account used for mail.
    ChooseAccount {
        /// The accounts used for mail.
        accounts: Vec<LearnAccount>,
        /// The one chosen, if any.
        selected: Option<String>,
    },
    /// Which sent mail.
    ChooseRange {
        /// The address learned from.
        address: String,
        /// What can be chosen.
        ranges: LearnRanges,
        /// The range chosen.
        selected: Option<LearnRange>,
        /// How far back the device holds the account's mail.
        reach: SentReach,
    },
    /// What the range holds on this device, and what is sent where.
    Review {
        /// The address learned from.
        address: String,
        /// The range read.
        range: LearnRange,
        /// How far back the device holds the account's mail.
        reach: SentReach,
        /// What was found.
        review: Review,
        /// Where Learn would send, or why it cannot; `None` while nothing found is usable, since
        /// nothing will be sent.
        sends_to: Option<Result<AiRoute, AiFailure>>,
        /// Whether the account's server keeps the drafted keyword; without it only this device
        /// knows which replies were drafted.
        keyword_kept: bool,
    },
    /// The run, one request per language and part.
    Learning {
        /// Requests answered.
        done: u32,
        /// Requests the run makes; zero until it knows.
        total: u32,
    },
    /// No style was learned.
    Failed {
        /// Why.
        failure: AiFailure,
        /// Its remedy (`LearningIntent::Remedy` where it is a step of the flow).
        remedy: Option<RemedyControl>,
    },
    /// A style was learned; the reveal follows in the same host.
    Learned {
        /// The new style.
        style: WritingStyleId,
    },
}

/// The learning flow, as a host draws it. Each control is `None` where the stage has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningView {
    /// The stage.
    pub stage: LearningStage,
    /// The stages before it, first to last, each where Back from the one after it goes.
    pub trail: Vec<LearningTrailStage>,
    /// Back.
    pub back: Option<StyleControl>,
    /// Next, on `ChooseAccount` and `ChooseRange`.
    pub next: Option<StyleControl>,
    /// Learn, on `Review`: pressing it is the consent.
    pub learn: Option<StyleControl>,
    /// Stop, on `Learning`.
    pub stop: Option<StyleControl>,
    /// Closing the host.
    pub close: StyleControl,
}

/// What an intent asks the host to start, off its own thread.
pub enum LearningStep<P> {
    /// Nothing.
    Nothing,
    /// [`App::read_learning_review`] with this ticket, on the host's runtime.
    Read(u64),
    /// This work, through [`App::queue_ai_work`].
    Learn(AiWork<P>),
    /// This change to the library, through [`App::queue_writing_style_change`].
    Change(WritingStyleChange),
}

impl<P> std::fmt::Debug for LearningStep<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing => f.write_str("Nothing"),
            Self::Read(ticket) => f.debug_tuple("Read").field(ticket).finish(),
            Self::Learn(work) => f.debug_tuple("Learn").field(work).finish(),
            Self::Change(change) => f.debug_tuple("Change").field(change).finish(),
        }
    }
}

/// The control a stage draws.
const fn control(enabled: bool) -> StyleControl {
    StyleControl {
        enabled,
        running: false,
    }
}

impl<P: Provider> App<P> {
    fn flows(&self) -> std::sync::MutexGuard<'_, Flows> {
        self.ai
            .writing_style
            .flows
            .lock()
            .expect("learning flow mutex poisoned")
    }

    /// How far back the device holds `account`'s mail.
    fn sent_reach(&self, account: &AccountId) -> SentReach {
        SentReach::of(self.effective_sync_depth(account.as_str()), today())
    }

    /// The learning flow while a host draws it, or `None`. Read from memory.
    #[must_use]
    pub fn writing_style_learning(&self) -> Option<LearningView> {
        if !self.ai_offer() {
            return None;
        }
        let route = self.ai_route();
        let flows = self.flows();
        let flow = flows.flow.as_ref().filter(|flow| flow.shown)?;
        Some(self.learning_view(flow, route))
    }

    fn learning_view(&self, flow: &Flow, route: Result<AiRoute, AiFailure>) -> LearningView {
        let address = flow
            .account
            .as_ref()
            .map(|(_, address)| address.clone())
            .unwrap_or_default();
        let reach = flow
            .account
            .as_ref()
            .map_or(SentReach::Everything, |(id, _)| self.sent_reach(id));
        let mut view = LearningView {
            stage: LearningStage::Learning { done: 0, total: 0 },
            trail: flow.trail(),
            back: None,
            next: None,
            learn: None,
            stop: None,
            close: StyleControl {
                enabled: true,
                running: false,
            },
        };
        view.stage = match &flow.stage {
            Stage::ChooseAccount => {
                view.next = Some(control(flow.account.is_some()));
                LearningStage::ChooseAccount {
                    accounts: self
                        .accounts
                        .senders()
                        .into_iter()
                        .map(|(id, address)| LearnAccount {
                            id: id.as_str().to_owned(),
                            address,
                        })
                        .collect(),
                    selected: flow.account.as_ref().map(|(id, _)| id.as_str().to_owned()),
                }
            }
            Stage::ChooseRange => {
                let ranges = learn_ranges(reach, today());
                view.back = flow.asked_account.then_some(StyleControl {
                    enabled: true,
                    running: false,
                });
                view.next = Some(control(
                    flow.range.is_some_and(|range| ranges.admits(range)),
                ));
                LearningStage::ChooseRange {
                    address,
                    ranges,
                    selected: flow.range,
                    reach,
                }
            }
            Stage::Review(review) => {
                view.back = Some(control(true));
                view.learn = Some(control(matches!(review, Review::Ready(_)) && route.is_ok()));
                LearningStage::Review {
                    address,
                    range: flow.range.unwrap_or(LearnRange::Everything),
                    reach,
                    review: review.clone(),
                    sends_to: (!matches!(review, Review::NothingUsable(_))).then_some(route),
                    keyword_kept: flow
                        .reviewed
                        .as_ref()
                        .is_none_or(|reviewed| reviewed.keyword_kept),
                }
            }
            Stage::Learning { done, total } => {
                view.stop = Some(control(true));
                LearningStage::Learning {
                    done: *done,
                    total: *total,
                }
            }
            Stage::Failed(failure) => {
                view.back = Some(control(true));
                LearningStage::Failed {
                    failure: *failure,
                    remedy: failure.remedy_control(true),
                }
            }
            Stage::Learned(style) => LearningStage::Learned {
                style: style.clone(),
            },
        };
        view
    }

    /// The run under way for the page to draw in Learn's place: its progress, or the failure it
    /// came to after its host was closed.
    /// Its remedy is held while `busy`, a change to the library running.
    pub(super) fn learning_on_page(&self, busy: bool) -> Option<PageRun> {
        let flows = self.flows();
        let flow = flows.flow.as_ref()?;
        match flow.stage {
            Stage::Learning { done, total } => Some(PageRun::Learning {
                done,
                total,
                stop: StyleControl::of(false, false),
            }),
            Stage::Failed(failure) if !flow.shown => Some(PageRun::Failed {
                failure,
                remedy: failure.remedy_control(!busy),
            }),
            _ => None,
        }
    }

    /// Moves the flow by `intent` and signals [`Surface::WritingStyle`]. Returns at once with
    /// what the host is to start off its own thread.
    pub fn writing_style_learning_intent(&self, intent: LearningIntent) -> LearningStep<P> {
        if !self.ai_offer() {
            return LearningStep::Nothing;
        }
        let accounts = self.accounts.senders();
        let route = self.ai_route().map(|_| ());
        let stopping = intent == LearningIntent::Stop;
        let today = today();
        let start = self.flows().apply(
            intent,
            &accounts,
            |account, range| learn_ranges(self.sent_reach(account), today).admits(range),
            route,
        );
        if stopping {
            self.cancel_ai_work(&AiWorkKey::Learning);
        }
        self.observer.surface_changed(Surface::WritingStyle);
        match start {
            Start::Nothing => LearningStep::Nothing,
            Start::Read(ticket) => LearningStep::Read(ticket),
            Start::Learn(ticket) => LearningStep::Learn(AiWork::new(
                AiWorkKey::Learning,
                move |app: &Self, run: &AiRun<'_>| app.run_learning(ticket, run),
            )),
            Start::RevealClosed(id) => LearningStep::Change(WritingStyleChange::CloseReveal { id }),
        }
    }

    /// Reads what the review under `ticket` found, on the device; an answer for a review the
    /// person has since left is dropped. Sends nothing.
    pub async fn read_learning_review(&self, ticket: u64) {
        let asked = self.flows().current(ticket).and_then(|flow| {
            (flow.stage == Stage::Review(Review::Reading))
                .then(|| Some((flow.account.clone()?.0, flow.range?)))
                .flatten()
        });
        let Some((account, range)) = asked else {
            return;
        };
        let another_account = self.accounts.senders().len() > 1;
        let read = self
            .learnable_sent_mail(&account, range.bounds(today()))
            .await;
        let keyword_kept = self.keeps_drafted_keyword(&account).await;
        let found = read.map(|(messages, horizon)| {
            let signatures = self.signature_plain_texts();
            corpus::build(
                messages,
                &CorpusOptions {
                    signatures,
                    horizon,
                },
            )
        });
        {
            let mut flows = self.flows();
            let Some(flow) = flows
                .current(ticket)
                .filter(|flow| flow.stage == Stage::Review(Review::Reading))
            else {
                return;
            };
            match found {
                None => flow.stage = Stage::Failed(AiFailure::NoSentFolder { another_account }),
                Some(corpus) if corpus.languages.values().all(Vec::is_empty) => {
                    flow.stage = Stage::Review(Review::NothingUsable(corpus.report));
                }
                Some(corpus) => {
                    flow.stage = Stage::Review(Review::Ready(corpus.report.clone()));
                    flow.reviewed = Some(Reviewed {
                        corpus,
                        keyword_kept,
                    });
                }
            }
        }
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// The plain text of every signature in the library, which the corpus cuts from a message
    /// where its client wrote no delimiter above it.
    fn signature_plain_texts(&self) -> Vec<String> {
        self.signatures
            .lock()
            .expect("signatures mutex poisoned")
            .library
            .entries
            .values()
            .map(|signature| signature.body_plain.clone())
            .collect()
    }

    /// The run the consent under `ticket` queued: what the review read, sent behind the gate.
    fn run_learning(&self, ticket: u64, run: &AiRun<'_>) {
        let taken = self.flows().current(ticket).and_then(|flow| {
            matches!(flow.stage, Stage::Learning { .. })
                .then(|| {
                    Some((
                        flow.reviewed.as_ref()?.corpus.clone(),
                        flow.learn_as.clone()?,
                        flow.account.clone()?,
                    ))
                })
                .flatten()
        });
        let Some((corpus, (name, language), (account, address))) = taken else {
            return;
        };
        let backend = match self.ai_backend(run) {
            Ok(backend) => backend,
            Err(failure) => return self.learning_ended(ticket, Err(failure)),
        };
        if corpus.languages.values().all(Vec::is_empty) {
            return self.learning_ended(ticket, Err(AiFailure::NothingToLearn));
        }
        let progress = |step: LearnProgress| {
            if let Some(flow) = self.flows().current(ticket)
                && matches!(flow.stage, Stage::Learning { .. })
            {
                flow.stage = Stage::Learning {
                    done: step.done,
                    total: step.total,
                };
            }
            self.observer.surface_changed(Surface::WritingStyle);
        };
        let learned = learn_style(
            &corpus,
            &backend,
            &LearnOptions {
                ui_language: &language,
                now: time::OffsetDateTime::now_utc().unix_timestamp(),
                cancel: run.cancel().as_atomic(),
                progress: &progress,
            },
        );
        match learned {
            Err(failed) => {
                if let Some(failure) = AiFailure::of_request(&failed.error, backend.destination()) {
                    self.learning_ended(ticket, Err(failure));
                }
            }
            Ok(_) if run.cancelled() => {}
            Ok(learned) => {
                let style = StoredWritingStyle {
                    name,
                    learned_from: address,
                    guide_json: serde_json::to_string(&learned.guide).unwrap_or_default(),
                    exemplars_json: serde_json::to_string(&learned.exemplars).unwrap_or_default(),
                    revealed: false,
                };
                let id = self.add_writing_style(Some(account.as_str()), style);
                self.learning_ended(ticket, Ok(id));
            }
        }
    }

    fn learning_ended(&self, ticket: u64, result: Result<WritingStyleId, AiFailure>) {
        {
            let mut flows = self.flows();
            let Some(flow) = flows.current(ticket) else {
                return;
            };
            match result {
                Ok(id) if flow.shown => flow.stage = Stage::Learned(id),
                // Its row opens the reveal, which has not been shown.
                Ok(_) => flows.flow = None,
                Err(failure) => {
                    flow.stage = Stage::Failed(failure);
                    flow.run_failed = true;
                }
            }
        }
        self.observer.surface_changed(Surface::WritingStyle);
    }

    /// Ends a flow that is showing the reveal of `id`, which has just been left.
    pub(super) fn reveal_left(&self, id: &WritingStyleId) {
        let mut flows = self.flows();
        if flows
            .flow
            .as_ref()
            .is_some_and(|flow| flow.stage == Stage::Learned(id.clone()))
        {
            flows.flow = None;
        }
    }
}
