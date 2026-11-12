//! The subscription behind an Allodia account, which the core reads, keeps and writes
//! (`MailcalApp::allodia_subscription_card`): this sends its intents, opens the pages it names,
//! and words what a write came to.
//!
//! **Linux sells through Allodia's own checkout and nothing else** (`purchasing.md`): Flatpak has
//! no commerce, so buying is a page opened in a browser. A store's subscription is read only: one
//! bought on a phone shows up here, and the section offers that store's page.

use mailcal_bindings::{
    AllodiaIntent, AllodiaPlan, AllodiaSubscriptionOutcome, AllodiaSubscriptionState, Intent,
};

use super::{AppModel, allodia_subscription_facts as facts, oauth_loopback, timestamps};
use crate::l10n;

/// Everything the subscription section asks the model for.
#[derive(Debug)]
pub(crate) enum SubscriptionInput {
    /// The section is drawn with nothing read yet.
    Refresh,
    /// The Settings window came back to the front, which on this client is what returning from
    /// the checkout browser looks like.
    Returned,
    Write(SubscriptionWrite),
    /// A store's own subscription page, the only thing that can change what that store bills.
    OpenStorePage(String),
}

/// Which write somebody asked for.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SubscriptionWrite {
    Checkout(AllodiaPlan),
    Cancel,
    Resubscribe,
    Switch(AllodiaPlan),
}

impl SubscriptionWrite {
    fn intent(self) -> AllodiaIntent {
        match self {
            Self::Checkout(plan) => AllodiaIntent::StartCheckout { plan },
            Self::Cancel => AllodiaIntent::CancelSubscription,
            Self::Resubscribe => AllodiaIntent::Resubscribe,
            Self::Switch(plan) => AllodiaIntent::SwitchInterval { plan },
        }
    }
}

impl AppModel {
    /// One piece of the section's traffic.
    pub(super) fn allodia_subscription_input(&mut self, input: SubscriptionInput) {
        let intent = match input {
            SubscriptionInput::Refresh => AllodiaIntent::RefreshSubscription,
            // Only a section that has already answered is one somebody is looking at, so a
            // focus change elsewhere costs no round trip.
            SubscriptionInput::Returned => {
                let answered = self
                    .app
                    .as_ref()
                    .is_some_and(|app| worth_rereading(&app.allodia_subscription_card().state));
                if !answered {
                    return;
                }
                AllodiaIntent::RefreshSubscription
            }
            SubscriptionInput::Write(write) => write.intent(),
            // Straight to the desktop's own launcher: a page, nothing is pending and nothing
            // comes back.
            SubscriptionInput::OpenStorePage(url) => {
                oauth_loopback::launch_browser(&url, || {
                    log::warn!("allodia: the store's subscription page could not be opened");
                });
                return;
            }
        };
        self.dispatch(Intent::Allodia { intent });
    }

    /// Opens the payment page the last checkout or restart named, once.
    ///
    /// ⚠️ **The browser, never a web view**: the page carries the payment-method choice and the
    /// authorisation for a recurring charge. Never the URL in a log line: it is a payment session
    /// minted for this person.
    pub(super) fn open_checkout_page(&mut self) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let Some(page) = app
            .allodia_subscription_card()
            .checkout_page
            .filter(|page| page.id > self.settings.allodia_checkout_opened)
        else {
            return;
        };
        self.settings.allodia_checkout_opened = page.id;
        oauth_loopback::launch_browser(&page.url, || {
            log::warn!("allodia: the checkout page could not be opened");
        });
    }
}

/// Whether coming back to the window is worth asking the service again: only where an answer is
/// drawn. A read still out answers for the return, and the core drops the second.
pub(crate) fn worth_rereading(state: &AllodiaSubscriptionState) -> bool {
    matches!(
        state,
        AllodiaSubscriptionState::Unavailable
            | AllodiaSubscriptionState::NeedsReauth
            | AllodiaSubscriptionState::Loaded { .. }
    )
}

/// What a write came to, in the reader's language.
pub(crate) fn outcome_text(outcome: &AllodiaSubscriptionOutcome, locale: &str) -> String {
    match outcome {
        // ⚠️ The date is the whole reassurance; one this build cannot read is still drawn, since
        // a sentence with no date at all is not true.
        AllodiaSubscriptionOutcome::Cancelled { end_date } => {
            l10n::settings_subscription_cancelled(
                &timestamps::account_date(end_date, locale).unwrap_or_else(|| end_date.clone()),
            )
        }
        AllodiaSubscriptionOutcome::Restarted => {
            l10n::settings_subscription_resubscribed().to_owned()
        }
        // The service's own figures, never today's list price.
        AllodiaSubscriptionOutcome::Switched {
            next_payment_date,
            amount_in_cents,
            currency,
        } => l10n::settings_subscription_switched(
            &next_payment_date
                .as_deref()
                .and_then(|date| timestamps::account_date(date, locale))
                .unwrap_or_default(),
            &facts::minor_units(*amount_in_cents, currency, locale),
        ),
        AllodiaSubscriptionOutcome::Failed { failure } => {
            facts::write_failure_text(*failure).to_owned()
        }
    }
}

#[cfg(test)]
#[path = "allodia_subscription_tests.rs"]
mod tests;
