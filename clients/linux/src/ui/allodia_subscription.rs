//! The subscription behind an Allodia account: one read that answers the whole section, and the
//! four writes that act on the subscription Allodia bills directly.
//!
//! The rules are `purchasing.md`, the contract beside the Allodia Licence. Two of them decide what
//! is here. **Linux sells through Allodia's own checkout and nothing else**: Flatpak has no
//! commerce, so there is no store purchase to attach, nothing to acknowledge, nothing left
//! outstanding on the device, and buying is a page opened in a browser. And **a store's
//! subscription is read only**: one bought on a phone shows up here, because the account is the
//! same account, but it is changed at that store and the section offers its page.
//!
//! ⚠️ **All five calls block**, each on a token round trip and a request, so each runs on a thread
//! and comes back as an input; the shape every other network call in this client takes. Each write
//! re-reads afterwards rather than editing the section's copy: the service recomputes every biller,
//! and only it knows what a write did.

use std::sync::Arc;

use mailcal_bindings::{AllodiaPlan, AllodiaSubscription, MailcalApp};

use super::{
    AppInput, AppModel,
    allodia_subscription_facts::{self as facts, WriteFailure},
    oauth_loopback, timestamps,
};
use crate::l10n;

/// Everything the subscription section asks the model for.
///
/// One input rather than five, so the dispatch next door has a single arm to grow: they are one
/// screen's traffic and every one of them ends in the same re-read.
#[derive(Debug)]
pub(crate) enum SubscriptionInput {
    /// Ask the service, once, because somebody opened the page that draws the answer.
    Read,
    ReadFinished(Box<SubscriptionAnswer>),
    Write(SubscriptionWrite),
    /// The Settings window came back to the front, which on this client is what returning from
    /// the checkout browser looks like.
    Returned,
    Written(Box<WriteOutcome>),
    /// A store's own subscription page, which is the only thing that can change a subscription
    /// that store is billing.
    OpenStorePage(String),
}

/// What one read learned.
///
/// The three arms are not interchangeable: a service that could not be reached costs a sentence, a
/// sign-in that predates the permission costs a sign-in, and only the third has anything to draw.
#[derive(Clone, Debug)]
pub(crate) enum SubscriptionAnswer {
    /// The read did not come back, or this deployment has no billing configured.
    Unavailable,
    /// This device's sign-in predates the permission the read needs. An offer rather than an
    /// error: they are signed in, one thing is asleep, and the ordinary sign-in asks for the full
    /// current scope set. Kept apart from [`Self::Unavailable`] because the remedies differ:
    /// waiting fixes an outage and never fixes this.
    NeedsReauth,
    /// The service answered.
    Loaded(Box<AllodiaSubscription>),
}

/// What a write answered, in the terms the section has to say it.
#[derive(Debug)]
pub(crate) enum WriteOutcome {
    /// The recurring charge stopped, with the day access runs to.
    Cancelled(String),
    /// Restarted on the authorisation already held: nothing to pay, nothing to re-enter.
    Restarted,
    /// The service handed back a payment page. It is opened in the browser on the main context,
    /// and the answer is on the next read.
    SentToBrowser(String),
    /// The next charge moved, with the service's own figures for it.
    Switched {
        next_payment_date: Option<String>,
        amount_in_cents: i64,
    },
    /// Nothing happened, and why as far as it can be said.
    Failed(WriteFailure),
}

/// Which write somebody asked for.
///
/// One input carrying four verbs rather than four inputs: they share every step but the call
/// itself, and the dispatch next door has one arm to grow rather than four.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SubscriptionWrite {
    Checkout(AllodiaPlan),
    Cancel,
    Resubscribe,
    Switch(AllodiaPlan),
}

/// Everything the section holds between renders.
#[derive(Clone, Debug, Default)]
pub(crate) struct SubscriptionState {
    /// A read is outstanding.
    pub(crate) checking: bool,
    /// What the last read learned, or `None` while nobody has asked yet, which is what starts one.
    /// The two are different things to draw: a spinner, and a failure.
    pub(crate) answer: Option<SubscriptionAnswer>,
    /// A write is outstanding, so every button that would start a second one is insensitive.
    pub(crate) writing: bool,
    /// The last thing worth saying about an attempt, already in the reader's language.
    pub(crate) note: Option<String>,
}

impl SubscriptionState {
    /// The answer, when there is one to draw.
    pub(crate) fn subscription(&self) -> Option<&AllodiaSubscription> {
        match &self.answer {
            Some(SubscriptionAnswer::Loaded(subscription)) => Some(subscription),
            _ => None,
        }
    }

    /// Whether coming back to the window is worth asking the service again.
    ///
    /// Only a section that has already answered: that is what says it is drawn at all, so the
    /// round trip is spent on the page that shows one rather than on every page this window has.
    /// A read already in flight will answer for the return anyway.
    pub(crate) fn worth_rereading(&self) -> bool {
        self.answer.is_some() && !self.checking
    }

    /// The currency the account is billed in, which the service sends with today's prices.
    ///
    /// ⚠️ It is the only place a client can learn it, because a switch reports an amount in minor
    /// units alone. Today's **list** currency, so it is the one this subscriber was charged in
    /// unless their billing currency has since moved, which the service does not currently do.
    fn currency(&self) -> &str {
        self.subscription()
            .map_or("", |subscription| subscription.prices.currency.as_str())
    }
}

impl AppModel {
    /// One piece of the section's traffic.
    pub(super) fn allodia_subscription_input(
        &mut self,
        input: SubscriptionInput,
        sender: relm4::Sender<AppInput>,
    ) {
        match input {
            SubscriptionInput::Read => self.read_allodia_subscription(sender),
            SubscriptionInput::ReadFinished(answer) => self.allodia_subscription_read(*answer),
            SubscriptionInput::Write(write) => self.write_allodia_subscription(write, sender),
            SubscriptionInput::Returned => {
                if self.settings.allodia_subscription.worth_rereading() {
                    self.read_allodia_subscription(sender);
                }
            }
            SubscriptionInput::Written(outcome) => {
                self.allodia_subscription_written(&outcome, sender);
            }
            // Straight to the desktop's own launcher: a store's page is a page, nothing is pending
            // and nothing comes back.
            SubscriptionInput::OpenStorePage(url) => {
                oauth_loopback::launch_browser(&url, || {
                    log::warn!("allodia: the store's subscription page could not be opened");
                });
            }
        }
    }

    /// Reads the whole section, once, when somebody opens the page that draws it.
    ///
    /// A network round trip, so a page nobody opened is not worth one. Never what gates a
    /// capability: the entitlement answers that, locally and without asking anybody.
    fn read_allodia_subscription(&mut self, sender: relm4::Sender<AppInput>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.settings.allodia_subscription.checking || app.allodia_account().is_none() {
            return;
        }
        self.settings.allodia_subscription.checking = true;
        self.refresh_settings_in_place();
        std::thread::spawn(move || {
            sender.emit(AppInput::AllodiaSubscription(
                SubscriptionInput::ReadFinished(Box::new(read(&app))),
            ));
        });
    }

    fn allodia_subscription_read(&mut self, answer: SubscriptionAnswer) {
        self.settings.allodia_subscription.checking = false;
        self.settings.allodia_subscription.answer = Some(answer);
        self.refresh_settings_in_place();
    }

    /// Runs one write, off the main thread.
    ///
    /// What may be asked for at all is `actions`, read where the buttons are drawn; nothing here
    /// second-guesses it. The note is cleared first, because the last write's sentence beside this
    /// one's outcome reads as one statement about the wrong thing.
    fn write_allodia_subscription(
        &mut self,
        write: SubscriptionWrite,
        sender: relm4::Sender<AppInput>,
    ) {
        let Some(app) = self.app.clone() else {
            return;
        };
        if self.settings.allodia_subscription.writing {
            return;
        }
        self.settings.allodia_subscription.writing = true;
        self.settings.allodia_subscription.note = None;
        self.refresh_settings_in_place();
        std::thread::spawn(move || {
            sender.emit(AppInput::AllodiaSubscription(SubscriptionInput::Written(
                Box::new(run(&app, write)),
            )));
        });
    }

    /// Folds in what the write did, then asks again.
    ///
    /// The answer is dropped rather than edited: the service recomputes every biller, and a
    /// section that edited its own copy would disagree with it.
    fn allodia_subscription_written(
        &mut self,
        outcome: &WriteOutcome,
        sender: relm4::Sender<AppInput>,
    ) {
        let state = &mut self.settings.allodia_subscription;
        state.writing = false;
        let locale = l10n::active_locale();
        let currency = state.currency().to_owned();
        state.note = note_for(outcome, locale, &currency);
        state.answer = None;
        // ⚠️ **The browser, never a web view**: this page carries the payment-method choice and the
        // authorisation for a recurring charge, which is the same reason RFC 8252 keeps an
        // authorisation request out of one. Launched here rather than on the write's thread,
        // because the portal hands the desktop a URI from the main context.
        if let WriteOutcome::SentToBrowser(url) = outcome {
            // Never the URL in a log line: it is a payment page minted for this person's account.
            oauth_loopback::launch_browser(url, || {
                log::warn!("allodia: the checkout page could not be opened");
            });
        }
        self.read_allodia_subscription(sender);
    }

    /// Forgets the section. Called on sign-out: a subscription belongs to an account, and there is
    /// nothing to say about one that has left.
    pub(super) fn forget_allodia_subscription(&mut self) {
        self.settings.allodia_subscription = SubscriptionState::default();
    }
}

/// One read, off the main thread.
fn read(app: &Arc<MailcalApp>) -> SubscriptionAnswer {
    match app.allodia_subscription() {
        Ok(subscription) => SubscriptionAnswer::Loaded(Box::new(subscription)),
        Err(mailcal_bindings::AllodiaPurchaseError::NeedsReauth) => SubscriptionAnswer::NeedsReauth,
        Err(error) => {
            // Deliberately quiet on screen: a read that did not arrive costs a sentence, never
            // access. The detail goes to the diagnostic log alone, where it names endpoints and
            // status codes and never an address or a secret.
            log::warn!("allodia: the subscription read did not come back ({error})");
            SubscriptionAnswer::Unavailable
        }
    }
}

/// One write, off the main thread, and the one line it leaves behind.
fn run(app: &Arc<MailcalApp>, write: SubscriptionWrite) -> WriteOutcome {
    let outcome = attempt(app, write);
    note_in_log(write, &outcome);
    outcome
}

/// What a write leaves in the diagnostic log.
///
/// ⚠️ **Without it a support log cannot tell that anything was ever asked of the subscription.**
/// Three of the four answer with something the card says out loud and then vanish with the next
/// read, and the fourth hands the person to a browser and hears nothing back, so a plan that
/// changed and a subscription that appeared from nowhere would both read as the service having
/// done it by itself. The verb, the period and how it ended, which is what
/// [`docs/logging.md`](../../../../docs/logging.md) allows: never an amount, never a date, and
/// never the page, whose URL is a payment session.
fn note_in_log(write: SubscriptionWrite, outcome: &WriteOutcome) {
    let asked = match write {
        SubscriptionWrite::Checkout(plan) => format!("a {plan:?} checkout"),
        SubscriptionWrite::Cancel => "a cancellation".to_owned(),
        SubscriptionWrite::Resubscribe => "a restart".to_owned(),
        SubscriptionWrite::Switch(plan) => format!("a change to {plan:?}"),
    };
    match outcome {
        WriteOutcome::Failed(failure) => {
            log::warn!("allodia: {asked} did not go through ({failure:?})");
        }
        WriteOutcome::SentToBrowser(_) => {
            log::info!("allodia: {asked} was opened in the browser");
        }
        _ => log::info!("allodia: {asked} was accepted"),
    }
}

fn attempt(app: &Arc<MailcalApp>, write: SubscriptionWrite) -> WriteOutcome {
    match write {
        SubscriptionWrite::Checkout(plan) => match app.start_allodia_checkout(plan) {
            Ok(checkout) => checkout_page(&checkout),
            Err(error) => WriteOutcome::Failed(facts::write_failure(&error)),
        },
        SubscriptionWrite::Cancel => match app.cancel_allodia_subscription() {
            Ok(cancellation) => WriteOutcome::Cancelled(cancellation.end_date),
            Err(error) => WriteOutcome::Failed(facts::write_failure(&error)),
        },
        SubscriptionWrite::Resubscribe => match app.resubscribe_to_allodia() {
            Ok(checkout) if checkout.reactivated => WriteOutcome::Restarted,
            Ok(checkout) => checkout_page(&checkout),
            Err(error) => WriteOutcome::Failed(facts::write_failure(&error)),
        },
        SubscriptionWrite::Switch(plan) => match app.switch_allodia_interval(plan) {
            Ok(change) => WriteOutcome::Switched {
                next_payment_date: change.next_payment_date,
                amount_in_cents: change.amount_in_cents,
            },
            Err(error) => WriteOutcome::Failed(facts::write_failure(&error)),
        },
    }
}

/// The page the service handed back, if it handed one back at all.
///
/// A checkout with no URL is a service answer this build cannot act on; the re-read that follows
/// every write is what will show whatever it did decide.
fn checkout_page(checkout: &mailcal_bindings::AllodiaCheckout) -> WriteOutcome {
    checkout
        .checkout_url
        .as_deref()
        .filter(|url| !url.is_empty())
        .map_or(WriteOutcome::Failed(WriteFailure::Unexplained), |url| {
            WriteOutcome::SentToBrowser(url.to_owned())
        })
}

/// What an answered write is allowed to put on screen.
fn note_for(outcome: &WriteOutcome, locale: &str, currency: &str) -> Option<String> {
    match outcome {
        WriteOutcome::Cancelled(end_date) => Some(l10n::settings_subscription_cancelled(
            &timestamps::account_date(end_date, locale).unwrap_or_else(|| end_date.clone()),
        )),
        WriteOutcome::Restarted => Some(l10n::settings_subscription_resubscribed().to_owned()),
        // The browser has the rest of it, and the re-read is what will report the outcome.
        WriteOutcome::SentToBrowser(_) => None,
        WriteOutcome::Switched {
            next_payment_date,
            amount_in_cents,
        } => Some(l10n::settings_subscription_switched(
            &next_payment_date
                .as_deref()
                .and_then(|date| timestamps::account_date(date, locale))
                .unwrap_or_default(),
            &facts::minor_units(*amount_in_cents, currency, locale),
        )),
        WriteOutcome::Failed(failure) => Some(facts::write_failure_text(*failure).to_owned()),
    }
}

#[cfg(test)]
#[path = "allodia_subscription_tests.rs"]
mod tests;
