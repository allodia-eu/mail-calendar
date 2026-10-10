//! The step a sign-in ends on when only the server could say what the account offers: a JMAP
//! session offering more than one of mail, calendar and contacts (`docs/onboarding.md`). The
//! choices are the core's (`signed_in_setup_choices`), drawn with the found card's toggles.

use adw::prelude::*;
use mailcal_bindings::{AccountCapability, SetupChoice};

use super::{
    AppInput,
    setup_uses::{self, UseOffer},
    setup_widgets::{body, caption, heading, page, primary},
};
use crate::l10n;

/// The step for one account just added.
#[derive(Debug)]
pub(crate) struct UsesStep {
    pub(super) account: String,
    /// The account whose link step comes after this one: the one an "Add another account" left,
    /// or this account.
    pub(super) next: String,
    pub(super) offer: UseOffer,
}

impl UsesStep {
    /// The step for `account`, or `None` when the core offers nothing to choose.
    pub(super) fn new(account: String, next: String, choices: Vec<SetupChoice>) -> Option<Self> {
        (!choices.is_empty()).then(|| Self {
            account,
            next,
            offer: UseOffer {
                choices,
                ..UseOffer::default()
            },
        })
    }

    /// The uses to switch off: offered, on now, and not chosen.
    pub(super) fn dropped(&self, chosen: &[AccountCapability]) -> Vec<AccountCapability> {
        self.offer
            .choices
            .iter()
            .filter(|choice| choice.on && !chosen.contains(&choice.capability))
            .map(|choice| choice.capability)
            .collect()
    }
}

/// The step's page.
pub(super) fn step(
    window: &gtk::Window,
    step: &UsesStep,
    sender: &relm4::Sender<AppInput>,
) -> gtk::Box {
    let content = page();
    content.append(&heading(l10n::setup_signed_in_uses_title()));
    content.append(&body(l10n::setup_signed_in_uses_note()));
    let toggles = setup_uses::append(&content, &step.offer, &[]);
    content.append(&caption(l10n::setup_links_note()));

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_halign(gtk::Align::End);
    let done = primary(l10n::setup_detect_action(), window);
    let input = sender.clone();
    done.connect_clicked(move |_| {
        input.emit(AppInput::SetupUsesChosen(
            toggles.chosen().uses.unwrap_or_default(),
        ));
    });
    actions.append(&done);
    content.append(&actions);
    content
}

#[cfg(test)]
#[path = "setup_signed_in_tests.rs"]
pub(super) mod tests;
