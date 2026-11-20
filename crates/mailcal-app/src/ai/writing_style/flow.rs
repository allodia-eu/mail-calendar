//! The learning flow's stages and how each intent moves them (`docs/ai.md` rules 11 to 13): a
//! state machine with no engine or request in it, so every cell of the contract's table is a
//! plain test. [`super::learning`] runs the reads and the run it asks for.
//!
//! Every read and every run is started under a ticket, and its answer lands only while the flow
//! still holds that ticket: going back, choosing again, stopping or closing takes a new one, so an
//! answer for a range or an account the person has since left is dropped.

use engine_api::AccountId;
use mailcal_account::WritingStyleId;
use mailcal_ai::{LanguageCode, corpus::Corpus};

use super::range::LearnRange;
use crate::ai::{AiFailure, AiRemedy};

/// What the person does in the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearningIntent {
    /// Opens the flow, or joins the one under way.
    Open,
    /// Chooses the account to learn from, on `ChooseAccount`.
    SelectAccount {
        /// The account's id.
        account: String,
    },
    /// Chooses the range, on `ChooseRange`.
    SelectRange {
        /// The range.
        range: LearnRange,
    },
    /// Moves on from `ChooseAccount` or `ChooseRange`.
    Next,
    /// Goes back, where the stage has a way back.
    Back,
    /// The consent: sends what the review found, under `name` and with descriptions in
    /// `language`, the client's words and language, which the core has none of.
    Learn {
        /// The name the style is stored under until the person gives it one.
        name: String,
        /// The language the person reads the app in, as an ISO 639-1 code.
        language: String,
    },
    /// Stops the run.
    Stop,
    /// Takes the failure's remedy: moves the flow for one that is a step of it, waits on the review
    /// for the Allodia sign-in, and ends the flow for the own endpoint's form.
    Remedy,
    /// Closes the host.
    Close,
}

/// What the review found on the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Review {
    /// Reading; nothing has left the device.
    Reading,
    /// Something to learn from.
    Ready(mailcal_ai::corpus::CorpusReport),
    /// Nothing that says enough.
    NothingUsable(mailcal_ai::corpus::CorpusReport),
}

/// Where the flow stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Stage {
    ChooseAccount,
    ChooseRange,
    Review(Review),
    Learning { done: u32, total: u32 },
    Failed(AiFailure),
    Learned(WritingStyleId),
}

/// A stage the flow can be taken back to, as a trail names it: an identifier a navigation path
/// keys on, carrying none of the stage's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningTrailStage {
    /// `ChooseAccount`.
    ChooseAccount,
    /// `ChooseRange`.
    ChooseRange,
    /// `Review`.
    Review,
}

/// What the review read, kept so the run sends exactly it and Stop returns to it.
pub(super) struct Reviewed {
    pub(super) corpus: Corpus,
    pub(super) keyword_kept: bool,
}

/// One flow.
pub(super) struct Flow {
    /// Whether a host draws it. Closed while a run goes, the page draws the run instead.
    pub(super) shown: bool,
    /// Whether `ChooseAccount` was shown, which is where `ChooseRange` goes back to.
    pub(super) asked_account: bool,
    /// The account chosen, with its address.
    pub(super) account: Option<(AccountId, String)>,
    pub(super) range: Option<LearnRange>,
    pub(super) stage: Stage,
    pub(super) reviewed: Option<Reviewed>,
    /// The name and language the last Learn gave, for Try again.
    pub(super) learn_as: Option<(String, LanguageCode)>,
    /// Whether the failure shown came from the run rather than the read.
    pub(super) run_failed: bool,
}

/// The flow, and the ticket its reads and runs answer under.
#[derive(Default)]
pub(crate) struct Flows {
    pub(super) flow: Option<Flow>,
    pub(super) ticket: u64,
}

/// What an intent asks the caller to start.
pub(super) enum Start {
    Nothing,
    /// Read the review under this ticket.
    Read(u64),
    /// Run the learning under this ticket.
    Learn(u64),
    /// The reveal of this style is left by closing, as `WritingStyleChange::CloseReveal` leaves
    /// it.
    RevealClosed(WritingStyleId),
}

impl Flows {
    fn next_ticket(&mut self) -> u64 {
        self.ticket += 1;
        self.ticket
    }

    /// Opens a flow over `accounts`, the accounts used for mail, or joins the one there is.
    pub(super) fn open(&mut self, accounts: &[(AccountId, String)]) {
        if let Some(flow) = &mut self.flow {
            flow.shown = true;
            return;
        }
        let (stage, account) = match accounts {
            [] => return,
            [only] => (Stage::ChooseRange, Some(only.clone())),
            _ => (Stage::ChooseAccount, None),
        };
        self.next_ticket();
        self.flow = Some(Flow {
            shown: true,
            asked_account: accounts.len() > 1,
            account,
            range: Some(LearnRange::Everything),
            stage,
            reviewed: None,
            learn_as: None,
            run_failed: false,
        });
    }

    /// Moves the flow by `intent`. `accounts` are the accounts used for mail, `admits` says
    /// whether the chosen account can honour a range, and `route` is where a request would go.
    pub(super) fn apply(
        &mut self,
        intent: LearningIntent,
        accounts: &[(AccountId, String)],
        admits: impl Fn(&AccountId, LearnRange) -> bool,
        route: Result<(), AiFailure>,
    ) -> Start {
        if intent == LearningIntent::Open {
            self.open(accounts);
            return Start::Nothing;
        }
        let Some(flow) = self.flow.as_mut() else {
            return Start::Nothing;
        };
        let before = flow.stage.clone();
        let start = match (intent, before.clone()) {
            (LearningIntent::SelectAccount { account }, Stage::ChooseAccount) => {
                if let Some(chosen) = accounts.iter().find(|(id, _)| id.as_str() == account)
                    && flow.account.as_ref() != Some(chosen)
                {
                    flow.account = Some(chosen.clone());
                    flow.range = Some(LearnRange::Everything);
                    flow.reviewed = None;
                }
                Start::Nothing
            }
            (LearningIntent::SelectRange { range }, Stage::ChooseRange) => {
                if let Some((account, _)) = &flow.account
                    && admits(account, range)
                    && flow.range != Some(range)
                {
                    flow.range = Some(range);
                    flow.reviewed = None;
                }
                Start::Nothing
            }
            (LearningIntent::Next, Stage::ChooseAccount) if flow.account.is_some() => {
                flow.stage = Stage::ChooseRange;
                Start::Nothing
            }
            (LearningIntent::Next, Stage::ChooseRange) if flow.range.is_some() => flow.read(route),
            (LearningIntent::Back, _) => {
                flow.back();
                Start::Nothing
            }
            (LearningIntent::Learn { name, language }, Stage::Review(Review::Ready(_))) => {
                let language = LanguageCode::new(&language)
                    .or_else(|| LanguageCode::new("en"))
                    .expect("en is a language code");
                flow.learn_as = Some((name, language));
                flow.learn(route)
            }
            (LearningIntent::Stop, Stage::Learning { .. }) => {
                if flow.shown {
                    flow.stage = flow.review_stage();
                } else {
                    self.flow = None;
                }
                Start::Nothing
            }
            (LearningIntent::Remedy, Stage::Failed(failure)) => match failure.remedy() {
                // The sign-in is taken over the flow, which waits where Learn can be pressed
                // again once the person is back.
                Some(
                    AiRemedy::ChooseAccount | AiRemedy::ChooseRange | AiRemedy::SignInToAllodia,
                ) => {
                    flow.back();
                    Start::Nothing
                }
                // The endpoint is set up under Settings → Advanced, which the flow's host gives
                // way to: the flow ends, as Close ends it.
                Some(AiRemedy::OpenAiEndpoint) => {
                    self.flow = None;
                    Start::Nothing
                }
                Some(AiRemedy::TryAgain) if flow.run_failed && flow.reviewed.is_some() => {
                    flow.learn(route)
                }
                Some(AiRemedy::TryAgain) => flow.read(route),
                _ => Start::Nothing,
            },
            // The run goes on without a host; the page draws it.
            (LearningIntent::Close, Stage::Learning { .. }) => {
                flow.shown = false;
                Start::Nothing
            }
            // Leaving the reveal is one change whichever Close arrives; the flow ends when it is
            // written, as it does for the reveal's own Close.
            (LearningIntent::Close, Stage::Learned(style)) => Start::RevealClosed(style),
            (LearningIntent::Close, _) => {
                self.flow = None;
                Start::Nothing
            }
            _ => Start::Nothing,
        };
        // A move leaves the read or run under way; what it starts answers under a new ticket.
        let moved = self.flow.as_ref().is_none_or(|flow| flow.stage != before);
        match start {
            Start::Read(_) => Start::Read(self.next_ticket()),
            Start::Learn(_) => Start::Learn(self.next_ticket()),
            other => {
                if moved {
                    self.next_ticket();
                }
                other
            }
        }
    }

    /// The flow, while it still holds `ticket`.
    pub(super) fn current(&mut self, ticket: u64) -> Option<&mut Flow> {
        if self.ticket == ticket {
            self.flow.as_mut()
        } else {
            None
        }
    }
}

impl Flow {
    /// Where Back goes, as the contract's table says, or `None` where the stage has no Back.
    fn back_to(&self) -> Option<LearningTrailStage> {
        let to_account = if self.asked_account {
            LearningTrailStage::ChooseAccount
        } else {
            LearningTrailStage::ChooseRange
        };
        Some(match &self.stage {
            Stage::ChooseRange if self.asked_account => LearningTrailStage::ChooseAccount,
            Stage::Review(_) => LearningTrailStage::ChooseRange,
            Stage::Failed(failure) => match failure.remedy() {
                Some(AiRemedy::ChooseAccount) => to_account,
                Some(AiRemedy::ChooseRange) => LearningTrailStage::ChooseRange,
                _ if self.reviewed.is_some() => LearningTrailStage::Review,
                _ => LearningTrailStage::ChooseRange,
            },
            _ => return None,
        })
    }

    /// Goes back as the contract's table says.
    fn back(&mut self) {
        self.stage = match self.back_to() {
            Some(LearningTrailStage::ChooseAccount) => Stage::ChooseAccount,
            Some(LearningTrailStage::ChooseRange) => Stage::ChooseRange,
            Some(LearningTrailStage::Review) => self.review_stage(),
            None => return,
        };
    }

    /// The stages before the current one, first to last, each where Back from the one after it
    /// goes: the path a navigation stack holds under the current stage. A stage without a Back
    /// (`Learning`, `Learned`) stands on the review it was started from.
    pub(super) fn trail(&self) -> Vec<LearningTrailStage> {
        let before = match self.stage {
            Stage::Learning { .. } | Stage::Learned(_) => Some(LearningTrailStage::Review),
            _ => self.back_to(),
        };
        let mut trail = Vec::new();
        if self.asked_account && before.is_some() {
            trail.push(LearningTrailStage::ChooseAccount);
        }
        if matches!(
            before,
            Some(LearningTrailStage::ChooseRange | LearningTrailStage::Review)
        ) {
            trail.push(LearningTrailStage::ChooseRange);
        }
        if before == Some(LearningTrailStage::Review) {
            trail.push(LearningTrailStage::Review);
        }
        trail
    }

    /// The review as it was read last, or the range to read one from.
    fn review_stage(&self) -> Stage {
        self.reviewed
            .as_ref()
            .map_or(Stage::ChooseRange, |reviewed| {
                Stage::Review(Review::Ready(reviewed.corpus.report.clone()))
            })
    }

    /// Asks the gate where a request would go, then reads; nothing is read when it would go
    /// nowhere.
    fn read(&mut self, route: Result<(), AiFailure>) -> Start {
        self.run_failed = false;
        if let Err(failure) = route {
            self.stage = Stage::Failed(failure);
            return Start::Nothing;
        }
        self.reviewed = None;
        self.stage = Stage::Review(Review::Reading);
        Start::Read(0)
    }

    fn learn(&mut self, route: Result<(), AiFailure>) -> Start {
        self.run_failed = true;
        if let Err(failure) = route {
            self.stage = Stage::Failed(failure);
            return Start::Nothing;
        }
        self.stage = Stage::Learning { done: 0, total: 0 };
        Start::Learn(0)
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
