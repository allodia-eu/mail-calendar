//! Learning a style in the flow host (`docs/ai.md` rules 11 to 14), drawn from the core's
//! `writing_style_learning`: the stage it is on, and exactly the controls it gives. A control
//! that is `None` is not drawn. Once a style is learned, its reveal follows in the same host.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use mailcal_bindings::{
    AiRemedy, ControlState, StyleLearningIntent, StyleLearningStage, WritingStyleIntent,
    WritingStyleLearning, WritingStyleOutcome, WritingStyleReveal,
};

use super::{
    writing_style_reveal::{self as reveal, Reveal},
    writing_style_stages::{self as stages, Range, Review},
    writing_style_words as words,
};
use crate::{
    l10n,
    ui::flow_host::{FlowAction, FlowHost, FlowStep, Pressed},
};

/// What the flow needs from outside the host.
pub(super) struct Wiring {
    pub(super) send: stages::Send,
    pub(super) style: reveal::Send,
    /// The reveal of a style, read from the core.
    pub(super) reveal_of: Rc<dyn Fn(String) -> Option<WritingStyleReveal>>,
    /// Opens what a remedy names that the flow does not move to: the Allodia sign-in, the
    /// endpoint form. The flow is closed first.
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
        self.drawn.replace(Some(record));
        self.host.show(step);
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
            let (weak, host) = (Rc::downgrade(&shown), Rc::downgrade(&self.host));
            shown.on_reshow(Rc::new(move || {
                if let (Some(shown), Some(host)) = (weak.upgrade(), host.upgrade()) {
                    host.show(shown.step());
                }
            }));
            self.reveal.replace(Some(Rc::clone(&shown)));
            shown
        };
        self.host.show(shown.step());
    }

    fn step(self: &Rc<Self>, record: &WritingStyleLearning) -> FlowStep {
        let send = &self.wiring.send;
        let (tag, title, content, primary) = match &record.stage {
            StyleLearningStage::ChooseAccount { accounts, selected } => (
                "account",
                l10n::learn_account_title(),
                stages::choose_account(accounts, selected.as_deref(), send),
                self.action(
                    record.next,
                    l10n::wizard_next(),
                    true,
                    StyleLearningIntent::Next,
                ),
            ),
            StyleLearningStage::ChooseRange {
                email,
                presets,
                earliest,
                latest,
                selected,
                reach,
            } => (
                "range",
                l10n::learn_range_title(),
                stages::choose_range(
                    &Range {
                        email,
                        presets,
                        earliest: earliest.as_deref(),
                        latest,
                        selected: selected.as_ref(),
                        reach,
                    },
                    send,
                ),
                self.action(
                    record.next,
                    l10n::wizard_next(),
                    true,
                    StyleLearningIntent::Next,
                ),
            ),
            StyleLearningStage::Review {
                email,
                range,
                reach,
                review,
                sends_to,
                cannot_send,
                keyword_kept,
            } => (
                "review",
                l10n::writing_style_learn(),
                stages::review(&Review {
                    email,
                    range,
                    reach,
                    state: review,
                    sends_to: sends_to.as_ref(),
                    cannot_send: *cannot_send,
                    keyword_kept: *keyword_kept,
                }),
                self.action(
                    record.learn,
                    l10n::learn_consent_confirm(),
                    true,
                    StyleLearningIntent::Learn {
                        name: l10n::writing_style_default_name().to_owned(),
                        language: l10n::active_locale().to_owned(),
                    },
                ),
            ),
            StyleLearningStage::Learning { done, total } => (
                "learning",
                l10n::writing_style_learn(),
                stages::learning(*done, *total),
                record.stop.map(|stop| {
                    let send = Rc::clone(send);
                    FlowAction {
                        label: l10n::learn_stop().to_owned(),
                        control: stop,
                        suggested: false,
                        advances: false,
                        pressed: Rc::new(move || send(StyleLearningIntent::Stop)),
                    }
                }),
            ),
            StyleLearningStage::Failed { failure, remedy } => (
                "failed",
                l10n::writing_style_learn(),
                stages::failed(*failure),
                record
                    .remedy
                    .zip(*remedy)
                    .map(|(control, remedy)| self.remedy(control, remedy)),
            ),
            StyleLearningStage::Learned { .. } => unreachable!("a learned style is its reveal"),
        };
        FlowStep {
            tag,
            title: title.to_owned(),
            title_widget: None,
            content,
            back: record.back,
            primary,
            close: record.close,
            footer: None,
            start: None,
        }
    }

    /// A control the record gives, or nothing when it gives none.
    fn action(
        &self,
        control: Option<ControlState>,
        label: &str,
        advances: bool,
        intent: StyleLearningIntent,
    ) -> Option<FlowAction> {
        let control = control?;
        let send = Rc::clone(&self.wiring.send);
        Some(FlowAction {
            label: label.to_owned(),
            control,
            suggested: true,
            advances,
            pressed: Rc::new(move || send(clone_intent(&intent))),
        })
    }

    /// The failure's remedy: one the flow moves to is sent to it, any other is opened where it
    /// lives once the flow is closed.
    fn remedy(&self, control: ControlState, remedy: AiRemedy) -> FlowAction {
        let (send, open) = (
            Rc::clone(&self.wiring.send),
            Rc::clone(&self.wiring.open_remedy),
        );
        let pressed: Pressed = Rc::new(move || match remedy {
            AiRemedy::ChooseAccount | AiRemedy::ChooseRange | AiRemedy::TryAgain => {
                send(StyleLearningIntent::Remedy);
            }
            other => {
                send(StyleLearningIntent::Close);
                open(other);
            }
        });
        FlowAction {
            label: words::remedy(remedy).to_owned(),
            control,
            suggested: true,
            advances: false,
            pressed,
        }
    }
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
