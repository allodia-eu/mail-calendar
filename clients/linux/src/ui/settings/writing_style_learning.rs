//! Learning a style in the flow host (`docs/ai.md` rules 11 to 14), drawn from the core's
//! `writing_style_learning`: the stage it is on, and exactly the controls it gives. A control
//! that is `None` is not drawn. Once a style is learned, its reveal follows in the same host.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use mailcal_bindings::{
    AiRemedy, AiRemedyControl, ControlState, StyleLearningIntent, StyleLearningStage,
    StyleLearningTrailStage, WritingStyleIntent, WritingStyleLearning, WritingStyleOutcome,
    WritingStyleReveal,
};

use super::{
    writing_style_reveal::{self as reveal, Reveal},
    writing_style_stages::{self as stages, Range, Review},
    writing_style_words as words,
};
use crate::{
    l10n,
    ui::flow_host::{FlowAction, FlowHost, FlowLeave, FlowStep, FlowTrailStep, Pressed},
};

/// What the flow needs from outside the host.
pub(super) struct Wiring {
    pub(super) send: stages::Send,
    pub(super) style: reveal::Send,
    /// The reveal of a style, read from the core.
    pub(super) reveal_of: Rc<dyn Fn(String) -> Option<WritingStyleReveal>>,
    /// Opens what a remedy names that leads outside the flow: the Allodia sign-in, the endpoint
    /// form. The flow has taken the remedy first.
    pub(super) open_remedy: Rc<dyn Fn(AiRemedy)>,
}

pub(super) struct Learning {
    pub(super) host: Rc<FlowHost>,
    wiring: Wiring,
    drawn: RefCell<Option<WritingStyleLearning>>,
    reveal: RefCell<Option<Rc<Reveal>>>,
}

impl Learning {
    pub(super) fn new(wiring: Wiring) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let (back, close) = (weak.clone(), weak.clone());
            let host = FlowHost::new(
                Rc::new(move || {
                    if let Some(flow) = back.upgrade() {
                        (flow.wiring.send)(StyleLearningIntent::Back);
                    }
                }),
                Rc::new(move || {
                    if let Some(flow) = close.upgrade() {
                        flow.close();
                    }
                }),
            );
            Self {
                host,
                wiring,
                drawn: RefCell::new(None),
                reveal: RefCell::new(None),
            }
        })
    }

    /// Closing the host: a reveal on screen counts as shown, anything else is the flow's Close.
    pub(super) fn close(&self) {
        let learned = self
            .drawn
            .borrow()
            .as_ref()
            .and_then(|record| match &record.stage {
                StyleLearningStage::Learned { style } => Some(style.clone()),
                _ => None,
            });
        match learned {
            Some(style) => (self.wiring.style)(WritingStyleIntent::CloseReveal { style }),
            None => (self.wiring.send)(StyleLearningIntent::Close),
        }
    }

    /// Draws `record`, with what the page's last change came to for a reveal on screen.
    pub(super) fn draw(
        self: &Rc<Self>,
        record: WritingStyleLearning,
        outcome: Option<&WritingStyleOutcome>,
    ) {
        if let StyleLearningStage::Learned { style } = &record.stage {
            self.drawn.replace(Some(record.clone()));
            self.draw_reveal(style, outcome);
            return;
        }
        if self.drawn.borrow().as_ref() == Some(&record) {
            return;
        }
        let step = self.step(&record);
        let trail = trail(&record.trail);
        self.drawn.replace(Some(record));
        self.host.show(&trail, step);
    }

    fn draw_reveal(self: &Rc<Self>, style: &str, outcome: Option<&WritingStyleOutcome>) {
        let Some(record) = (self.wiring.reveal_of)(style.to_owned()) else {
            return;
        };
        let current = self
            .reveal
            .borrow()
            .clone()
            .filter(|shown| shown.id == style);
        let shown = if let Some(shown) = current {
            shown.update(record);
            shown.outcome(outcome);
            shown
        } else {
            let seen = outcome.map_or(0, |outcome| outcome.serial);
            let shown = Reveal::new(record, seen, Rc::clone(&self.wiring.style));
            let (weak, flow) = (Rc::downgrade(&shown), Rc::downgrade(self));
            shown.on_reshow(Rc::new(move || {
                if let (Some(shown), Some(flow)) = (weak.upgrade(), flow.upgrade()) {
                    flow.show_reveal(&shown);
                }
            }));
            self.reveal.replace(Some(Rc::clone(&shown)));
            shown
        };
        self.show_reveal(&shown);
    }

    /// The reveal stands on the trail the record gives it, the review it was learned from.
    fn show_reveal(&self, shown: &Rc<Reveal>) {
        let trail = self
            .drawn
            .borrow()
            .as_ref()
            .map(|record| trail(&record.trail))
            .unwrap_or_default();
        self.host.show(&trail, shown.step());
    }

    fn step(self: &Rc<Self>, record: &WritingStyleLearning) -> FlowStep {
        let send = &self.wiring.send;
        let (tag, content, primary) = match &record.stage {
            StyleLearningStage::ChooseAccount { accounts, selected } => (
                ACCOUNT,
                stages::choose_account(accounts, selected.as_deref(), send),
                self.action(record.next, l10n::wizard_next(), StyleLearningIntent::Next),
            ),
            StyleLearningStage::ChooseRange {
                email,
                presets,
                selected,
                reach,
                fetch,
                ..
            } => (
                RANGE,
                stages::choose_range(
                    &Range {
                        email,
                        presets,
                        selected: selected.as_ref(),
                        reach,
                        fetch: *fetch,
                    },
                    send,
                ),
                self.action(record.next, l10n::wizard_next(), StyleLearningIntent::Next),
            ),
            StyleLearningStage::Review {
                email,
                range,
                reach,
                older_not_fetched,
                state,
                sends_to,
                cannot_send,
                keyword_kept,
            } => (
                REVIEW,
                stages::review(&Review {
                    email,
                    range,
                    reach,
                    older_not_fetched: *older_not_fetched,
                    state,
                    sends_to: sends_to.as_ref(),
                    cannot_send: *cannot_send,
                    keyword_kept: *keyword_kept,
                }),
                self.action(
                    record.learn,
                    l10n::learn_consent_confirm(),
                    StyleLearningIntent::Learn {
                        name: l10n::writing_style_default_name().to_owned(),
                        language: l10n::active_locale().to_owned(),
                    },
                ),
            ),
            StyleLearningStage::Learning { done, total } => (
                "learning",
                stages::learning(*done, *total),
                record.stop.map(|stop| {
                    let send = Rc::clone(send);
                    FlowAction {
                        label: l10n::learn_stop().to_owned(),
                        control: stop,
                        suggested: false,
                        pressed: Rc::new(move || send(StyleLearningIntent::Stop)),
                    }
                }),
            ),
            StyleLearningStage::Failed { failure, remedy } => (
                "failed",
                stages::failed(*failure),
                remedy.map(|remedy| self.remedy(remedy)),
            ),
            StyleLearningStage::Learned { .. } => unreachable!("a learned style is its reveal"),
        };
        FlowStep {
            tag,
            title: l10n::writing_style_learn().to_owned(),
            title_widget: None,
            content,
            back: record.back,
            leave: FlowLeave {
                label: l10n::action_cancel().to_owned(),
                control: record.close,
            },
            own_back: None,
            primary,
            footer: None,
        }
    }

    /// A control the record gives, or nothing when it gives none.
    fn action(
        &self,
        control: Option<ControlState>,
        label: &str,
        intent: StyleLearningIntent,
    ) -> Option<FlowAction> {
        let control = control?;
        let send = Rc::clone(&self.wiring.send);
        Some(FlowAction {
            label: label.to_owned(),
            control,
            suggested: true,
            pressed: Rc::new(move || send(clone_intent(&intent))),
        })
    }

    /// The failure's remedy, sent to the flow whichever it is and never with a Close; one that
    /// leads elsewhere is then opened where it lives (`docs/ai.md` rules 11 and 27).
    fn remedy(&self, remedy: AiRemedyControl) -> FlowAction {
        let AiRemedyControl { remedy, control } = remedy;
        let (send, open) = (
            Rc::clone(&self.wiring.send),
            Rc::clone(&self.wiring.open_remedy),
        );
        let pressed: Pressed = Rc::new(move || {
            send(StyleLearningIntent::Remedy);
            open(remedy);
        });
        FlowAction {
            label: words::remedy(remedy).to_owned(),
            control,
            suggested: true,
            pressed,
        }
    }
}

const ACCOUNT: &str = "account";
const RANGE: &str = "range";
const REVIEW: &str = "review";

/// The pages under the stage on screen, as the record's trail names them.
fn trail(stages: &[StyleLearningTrailStage]) -> Vec<FlowTrailStep> {
    stages
        .iter()
        .map(|stage| FlowTrailStep {
            tag: match stage {
                StyleLearningTrailStage::ChooseAccount => ACCOUNT,
                StyleLearningTrailStage::ChooseRange => RANGE,
                StyleLearningTrailStage::Review => REVIEW,
            },
            title: l10n::writing_style_learn().to_owned(),
        })
        .collect()
}

/// The intents a press sends carry only strings, so a press can send its own again.
fn clone_intent(intent: &StyleLearningIntent) -> StyleLearningIntent {
    match intent {
        StyleLearningIntent::Learn { name, language } => StyleLearningIntent::Learn {
            name: name.clone(),
            language: language.clone(),
        },
        StyleLearningIntent::Next => StyleLearningIntent::Next,
        StyleLearningIntent::Back => StyleLearningIntent::Back,
        StyleLearningIntent::Stop => StyleLearningIntent::Stop,
        StyleLearningIntent::Remedy => StyleLearningIntent::Remedy,
        StyleLearningIntent::Close => StyleLearningIntent::Close,
        StyleLearningIntent::Open => StyleLearningIntent::Open,
        StyleLearningIntent::SelectAccount { account } => StyleLearningIntent::SelectAccount {
            account: account.clone(),
        },
        StyleLearningIntent::SelectRange { range } => StyleLearningIntent::SelectRange {
            range: range.clone(),
        },
    }
}
