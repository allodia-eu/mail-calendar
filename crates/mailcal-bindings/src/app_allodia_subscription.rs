// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The Allodia account's subscription section, run by the core (`purchasing.md`, "The account
//! screen"; `docs/waiting.md`): one read that answers the section, kept for the signed-in
//! account, and four writes that only ever touch **Allodia's own** subscription, each a
//! `AllodiaIntent` that returns at once. A store's subscription is changed at that store, and the
//! section names its page.
//!
//! Each call makes a token round trip and a request, so each runs on a thread of the core's own
//! and ends in `Surface::Allodia`. A write is followed by a read, because the service recomputes
//! every biller and only it knows what a write did; the write's control runs until that read
//! lands.

use std::sync::Arc;

use crate::{
    AllodiaPlan, AllodiaPurchaseError, AllodiaSubscriptionCard, AllodiaSubscriptionOutcome,
    AllodiaSubscriptionState, MailcalApp,
    allodia_subscription_card::{Answer, Write},
    allodia_subscription_facts as facts,
};

#[uniffi::export]
impl MailcalApp {
    /// The subscription section: read again after every `Surface::Allodia` signal. Local and
    /// cheap; the page that draws it sends `AllodiaIntent::RefreshSubscription` when it opens.
    ///
    /// ⚠️ **Not what gates a capability.** This is what the section *says*; what to switch on
    /// comes from the entitlement, which is cached and never blocks.
    #[must_use]
    pub fn allodia_subscription_card(&self) -> AllodiaSubscriptionCard {
        let hidden = AllodiaSubscriptionCard {
            state: AllodiaSubscriptionState::Hidden,
            outcome: None,
            checkout_page: None,
        };
        let Some(account) = self.allodia_account() else {
            return hidden;
        };
        if self.allodia_sign_in_ended().is_some() {
            return hidden;
        }
        let held = self.account_work.subscription.snapshot(&account.email);
        let state = match held.answer {
            None => AllodiaSubscriptionState::Checking,
            Some(Answer::Unavailable) => AllodiaSubscriptionState::Unavailable,
            Some(Answer::NeedsReauth) => AllodiaSubscriptionState::NeedsReauth,
            Some(Answer::Loaded(subscription)) => AllodiaSubscriptionState::Loaded {
                view: facts::view(&subscription, held.writing, held.reading),
            },
        };
        AllodiaSubscriptionCard {
            state,
            outcome: held.outcome,
            checkout_page: held.page,
        }
    }
}

impl MailcalApp {
    /// The signed-in account a read or write is for, when the section is drawn at all.
    fn subscription_account(&self) -> Option<String> {
        let account = self.allodia_account()?;
        self.allodia_sign_in_ended()
            .is_none()
            .then_some(account.email)
    }

    /// Reads the subscription because the page drawing it asked.
    pub(crate) fn refresh_subscription(self: Arc<Self>) {
        let Some(account) = self.subscription_account() else {
            return;
        };
        if let Some(ticket) = self.account_work.subscription.begin_read(&account) {
            self.spawn_read(account, ticket, false);
        }
    }

    /// Reads the subscription again after a sign-in, when it had been asked for.
    pub(crate) fn reread_subscription(self: Arc<Self>) {
        let Some(account) = self.subscription_account() else {
            return;
        };
        if self.account_work.subscription.asked(&account) {
            let ticket = self.account_work.subscription.begin_reread(&account);
            self.spawn_read(account, ticket, false);
        }
    }

    fn spawn_read(self: Arc<Self>, account: String, ticket: u64, ends_write: bool) {
        self.app.allodia_changed();
        std::thread::spawn(move || {
            let answer = self.read_answer();
            self.account_work
                .subscription
                .read(&account, ticket, answer);
            if ends_write {
                self.account_work.subscription.write_ended(&account);
            }
            self.app.allodia_changed();
        });
    }

    /// One read, on the caller's thread.
    fn read_answer(&self) -> Answer {
        match self.fetch_allodia_subscription() {
            Ok(subscription) => Answer::Loaded(Box::new(subscription)),
            Err(AllodiaPurchaseError::NeedsReauth) => Answer::NeedsReauth,
            Err(error) => {
                // The refused sign-in is the account card's to say; anything else costs a
                // sentence, never access.
                log::warn!("allodia: the subscription read did not come back ({error})");
                Answer::Unavailable
            }
        }
    }

    /// Runs `write` when the section offers it now, then reads again.
    pub(crate) fn write_subscription(self: Arc<Self>, write: Write) {
        let Some(account) = self.subscription_account() else {
            return;
        };
        let offered = match self.allodia_subscription_card().state {
            AllodiaSubscriptionState::Loaded { view } => facts::permits(&view, write),
            _ => false,
        };
        if !offered || !self.account_work.subscription.begin_write(&account, write) {
            log::info!("allodia: {} was not on offer", asked(write));
            return;
        }
        self.app.allodia_changed();
        std::thread::spawn(move || {
            let currency = self.held_currency(&account);
            let (outcome, page) = self.run_write(write, currency);
            self.account_work
                .subscription
                .wrote(&account, outcome, page);
            let ticket = self.account_work.subscription.begin_reread(&account);
            self.spawn_read(account, ticket, true);
        });
    }

    /// The currency the account is billed in, which the service sends only with today's prices.
    fn held_currency(&self, account: &str) -> String {
        match self.account_work.subscription.snapshot(account).answer {
            Some(Answer::Loaded(subscription)) => subscription.prices.currency,
            _ => String::new(),
        }
    }

    /// One write, on the caller's thread: what it came to and a payment page to open.
    ///
    /// ⚠️ Logged as the verb, the period and how it ended: never an amount, a date or the page,
    /// whose URL is a payment session minted for this person.
    fn run_write(
        &self,
        write: Write,
        currency: String,
    ) -> (Option<AllodiaSubscriptionOutcome>, Option<String>) {
        let failed = |error: &AllodiaPurchaseError| {
            let failure = facts::write_failure(error);
            log::warn!("allodia: {} did not go through ({failure:?})", asked(write));
            (Some(AllodiaSubscriptionOutcome::Failed { failure }), None)
        };
        let page = |url: Option<String>| match url.filter(|url| !url.is_empty()) {
            Some(url) => {
                log::info!("allodia: {} was opened in the browser", asked(write));
                (None, Some(url))
            }
            None => failed(&AllodiaPurchaseError::Unreachable),
        };
        let accepted = |outcome| {
            log::info!("allodia: {} was accepted", asked(write));
            (Some(outcome), None)
        };
        match write {
            Write::Checkout(plan) => match self.allodia_checkout(plan) {
                Ok(checkout) => page(checkout.checkout_url),
                Err(error) => failed(&error),
            },
            Write::Cancel => match self.allodia_cancel() {
                Ok(end_date) => accepted(AllodiaSubscriptionOutcome::Cancelled { end_date }),
                Err(error) => failed(&error),
            },
            Write::Resubscribe => match self.allodia_resubscribe() {
                Ok(checkout) if checkout.reactivated => {
                    accepted(AllodiaSubscriptionOutcome::Restarted)
                }
                Ok(checkout) => page(checkout.checkout_url),
                Err(error) => failed(&error),
            },
            Write::Switch(plan) => match self.allodia_switch(plan) {
                Ok(change) => accepted(AllodiaSubscriptionOutcome::Switched {
                    next_payment_date: change.next_payment_date,
                    amount_in_cents: change.amount_in_cents,
                    currency,
                }),
                Err(error) => failed(&error),
            },
        }
    }
}

/// What a write is called in the diagnostic log.
fn asked(write: Write) -> String {
    match write {
        Write::Checkout(plan) => format!("a {plan:?} checkout"),
        Write::Cancel => "a cancellation".to_owned(),
        Write::Resubscribe => "a restart".to_owned(),
        Write::Switch(plan) => format!("a change to {plan:?}"),
    }
}

/// The five calls in a build without the Allodia account: each refuses before any network.
#[cfg(not(feature = "allodia-license"))]
impl MailcalApp {
    #[expect(
        clippy::unused_self,
        reason = "the same signature as the build that reads"
    )]
    fn fetch_allodia_subscription(
        &self,
    ) -> Result<crate::allodia_subscription::AllodiaSubscription, AllodiaPurchaseError> {
        Err(AllodiaPurchaseError::Unavailable)
    }

    #[expect(
        clippy::unused_self,
        reason = "the same signature as the build that writes"
    )]
    fn allodia_checkout(
        &self,
        _plan: AllodiaPlan,
    ) -> Result<crate::allodia_subscription::AllodiaCheckout, AllodiaPurchaseError> {
        Err(AllodiaPurchaseError::Unavailable)
    }

    #[expect(
        clippy::unused_self,
        reason = "the same signature as the build that writes"
    )]
    fn allodia_cancel(&self) -> Result<String, AllodiaPurchaseError> {
        Err(AllodiaPurchaseError::Unavailable)
    }

    #[expect(
        clippy::unused_self,
        reason = "the same signature as the build that writes"
    )]
    fn allodia_switch(
        &self,
        _plan: AllodiaPlan,
    ) -> Result<crate::allodia_subscription::AllodiaIntervalChange, AllodiaPurchaseError> {
        Err(AllodiaPurchaseError::Unavailable)
    }

    #[expect(
        clippy::unused_self,
        reason = "the same signature as the build that writes"
    )]
    fn allodia_resubscribe(
        &self,
    ) -> Result<crate::allodia_subscription::AllodiaCheckout, AllodiaPurchaseError> {
        Err(AllodiaPurchaseError::Unavailable)
    }
}

#[cfg(feature = "allodia-license")]
mod calls {
    use allodia_license::{AccountService, Error, Feature};

    use super::{AllodiaPlan, AllodiaPurchaseError};
    use crate::{
        AllodiaGrantHealth, MailcalApp,
        allodia_subscription::{AllodiaCheckout, AllodiaIntervalChange, AllodiaSubscription},
        allodia_transport::HttpsTransport,
    };

    /// Everything a call needs, gathered once so each of the five reads down the page.
    ///
    /// **Each blocks**, on a token round trip and a request: called on a thread of the core's
    /// own.
    struct Ready {
        service: AccountService,
        transport: HttpsTransport,
        token: String,
    }

    impl MailcalApp {
        /// Mint a token, check the grant permits `feature`, and build the transport.
        ///
        /// The order matters and is the same as the sync pass's: the token first, because minting
        /// one is what learns the grant's scopes on an install that never recorded them, and the
        /// scope check before the first request, because that request cannot succeed and its
        /// refusal would read as the service being down.
        fn ready(&self, feature: Feature) -> Result<Ready, AllodiaPurchaseError> {
            let token = self
                .allodia_access_token("the subscription")
                .map_err(|_| self.allodia_mint_failure())?;
            if !self.allodia_grant_permits(feature) {
                self.note_allodia_health(AllodiaGrantHealth::NeedsReauth);
                return Err(AllodiaPurchaseError::NeedsReauth);
            }
            let transport = HttpsTransport::new(self.runtime.handle().clone())
                .map_err(|_| AllodiaPurchaseError::Unreachable)?;
            Ok(Ready {
                service: AccountService::new(allodia_license::host()),
                transport,
                token,
            })
        }

        pub(super) fn fetch_allodia_subscription(
            &self,
        ) -> Result<AllodiaSubscription, AllodiaPurchaseError> {
            let ready = self.ready(Feature::ReadSubscription)?;
            let subscription = ready
                .service
                .subscription(&ready.transport, &ready.token)
                .map_err(|error| self.classify_allodia_failure(&error))?;
            self.note_allodia_health(AllodiaGrantHealth::Ok);
            Ok(subscription.into())
        }

        pub(super) fn allodia_checkout(
            &self,
            plan: AllodiaPlan,
        ) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .start_checkout(&ready.transport, &ready.token, plan.into())
                .map(Into::into)
                .map_err(|error| self.classify_allodia_failure(&error))
        }

        /// What has been paid through, and so when access ends.
        pub(super) fn allodia_cancel(&self) -> Result<String, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .cancel_subscription(&ready.transport, &ready.token)
                .map(|cancelled| cancelled.end_date)
                .map_err(|error| self.classify_allodia_failure(&error))
        }

        pub(super) fn allodia_switch(
            &self,
            plan: AllodiaPlan,
        ) -> Result<AllodiaIntervalChange, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .switch_interval(&ready.transport, &ready.token, plan.into())
                .map(Into::into)
                .map_err(|error| self.classify_allodia_failure(&error))
        }

        pub(super) fn allodia_resubscribe(&self) -> Result<AllodiaCheckout, AllodiaPurchaseError> {
            let ready = self.ready(Feature::WriteSubscription)?;
            ready
                .service
                .resubscribe(&ready.transport, &ready.token)
                .map(Into::into)
                .map_err(|error| self.classify_allodia_failure(&error))
        }

        /// What a token that could not be minted means to a client.
        ///
        /// Read after the mint, which has already recorded what the token endpoint's answer said
        /// about the grant: a refusal there is a sign-in to make again, a grant narrower than this
        /// build wants is the offer to sign in again, and anything else taught nothing and is the
        /// outage it looks like.
        pub(crate) fn allodia_mint_failure(&self) -> AllodiaPurchaseError {
            if self.allodia.lock().expect("allodia account lock").is_none() {
                return AllodiaPurchaseError::NotSignedIn;
            }
            match self.allodia_health() {
                AllodiaGrantHealth::SignedOut => AllodiaPurchaseError::SignedOut,
                AllodiaGrantHealth::NeedsReauth => AllodiaPurchaseError::NeedsReauth,
                AllodiaGrantHealth::Ok => AllodiaPurchaseError::Unreachable,
            }
        }

        /// Turn one failure into what a client draws, and record what it says about the grant.
        ///
        /// **A refusal and an outage are different answers**, the distinction the entitlement
        /// contract is built on. A service that declined to act told us something about the
        /// subscription; a service that could not be reached told us nothing, and in particular
        /// nothing about the grant.
        pub(super) fn classify_allodia_failure(&self, error: &Error) -> AllodiaPurchaseError {
            match error {
                Error::Unauthorized => {
                    self.note_allodia_health(AllodiaGrantHealth::SignedOut);
                    AllodiaPurchaseError::SignedOut
                }
                Error::Refused(refusal) => {
                    self.note_allodia_health(AllodiaGrantHealth::Ok);
                    AllodiaPurchaseError::Refused {
                        reason: refusal.clone().into(),
                    }
                }
                _ => AllodiaPurchaseError::Unreachable,
            }
        }
    }
}

#[cfg(test)]
#[path = "app_allodia_subscription_tests.rs"]
mod tests;
