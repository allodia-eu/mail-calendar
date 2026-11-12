// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! Settings → Allodia account → Subscription, kept by the core (`purchasing.md`, "The account
//! screen"; `docs/waiting.md`): what the last read said, every write's control, and what the last
//! write came to. Read with `MailcalApp::allodia_subscription_card` after every
//! `Surface::Allodia` signal; a client words it, formats its dates and amounts, and keeps no step
//! of its own. Buying from a phone's store stays the client's: no Rust can call StoreKit or Play.

use std::sync::Mutex;

use crate::{
    AllodiaBiller, AllodiaPlan, AllodiaPrices, AllodiaStore, ControlState,
    allodia_subscription::AllodiaSubscription,
};

/// What the subscription section draws.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaSubscriptionCard {
    pub state: AllodiaSubscriptionState,
    /// What the last write came to, said under the section until the next write starts.
    pub outcome: Option<AllodiaSubscriptionOutcome>,
    /// The payment page the last checkout or restart handed back, opened once for each `id` in
    /// the **browser**, never a web view: it carries the payment-method choice and the
    /// authorisation for a recurring charge.
    pub checkout_page: Option<AllodiaCheckoutPage>,
}

/// Where the section stands.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaSubscriptionState {
    /// Nobody is signed in, or the service refused the sign-in: no section. The account card
    /// says so and carries the remedy.
    Hidden,
    /// Nothing read yet. The client's page sends `AllodiaIntent::RefreshSubscription` when it
    /// opens.
    Checking,
    /// The read did not come back, or this deployment has no billing configured. A sentence,
    /// never lost access: the entitlement gates, locally.
    Unavailable,
    /// This device's sign-in predates the permission the read needs: the offer to sign in again
    /// (`AllodiaIntent::SignIn`), not an error.
    NeedsReauth,
    /// The service answered.
    Loaded { view: AllodiaSubscriptionView },
}

/// What a read decided the section shows.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "three independent facts the section words, and whether a read runs"
)]
pub struct AllodiaSubscriptionView {
    /// A paid plan is in force: the section says who bills and until when. Otherwise it says the
    /// account is free and offers buying, from `checkout` or from the phone's own store.
    pub entitled: bool,
    /// Who is charging now, named as "billed by": Allodia's own billing first, then each store
    /// once. `None` while entitled is rare and draws no such line.
    pub billed_by: Option<AllodiaBiller>,
    /// ISO 8601, the furthest date any source has been paid through.
    pub period_end: Option<String>,
    /// Something will charge again: "renews on" rather than "runs until".
    pub renews: bool,
    /// A charge failed and is being retried. **Access continues**, so never drawn as a lapse.
    pub grace: bool,
    /// Every source charging for the same plan at once, when there is more than one. **Reported,
    /// never resolved**: each exit is offered and none is taken.
    pub duplicate_billing: Vec<AllodiaBiller>,
    /// One store page per store with something left to manage there, each a button opening it.
    pub store_pages: Vec<AllodiaStorePage>,
    /// ISO 8601: what has been paid for, which both confirmations name.
    pub paid_through: Option<String>,
    /// Moving Allodia's own subscription to the other period, asked first.
    pub switch: Option<AllodiaSwitch>,
    /// Starting a cancelled subscription again.
    pub resubscribe: Option<ControlState>,
    /// Stopping the recurring charge, asked first. Drawn as destructive.
    pub cancel: Option<ControlState>,
    /// Buying through Allodia's own checkout, in a browser: the builds that sell that way draw it.
    /// `None` while anything is charging or this deployment has no billing.
    pub checkout: Option<AllodiaCheckoutOffer>,
    /// A read is running beside this answer, which stays drawn meanwhile.
    pub refreshing: bool,
}

/// A store's own subscription page: the only thing that can change what that store bills.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaStorePage {
    pub store: AllodiaStore,
    pub url: String,
}

/// The switch to the other period.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaSwitch {
    pub to: AllodiaPlan,
    pub control: ControlState,
}

/// Allodia's checkout, priced from today's list. A client formats the minor units.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaCheckoutOffer {
    pub prices: AllodiaPrices,
    pub monthly: ControlState,
    pub yearly: ControlState,
}

/// The payment page to open once.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaCheckoutPage {
    pub id: u64,
    pub url: String,
}

/// What a write came to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaSubscriptionOutcome {
    /// The recurring charge stopped; access runs to `end_date` (ISO 8601).
    Cancelled { end_date: String },
    /// Restarted on the authorisation already held: nothing to pay, nothing to re-enter.
    Restarted,
    /// The next charge moved: `amount_in_cents` in `currency` on `next_payment_date`.
    Switched {
        next_payment_date: Option<String>,
        amount_in_cents: i64,
        currency: String,
    },
    /// Nothing changed.
    Failed { failure: AllodiaWriteFailure },
}

/// Why a write did not happen, as far as it can be said. A client words each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaWriteFailure {
    /// Nothing more can be said: an outage, a refused token, a reason this build has never heard
    /// of. The sentence says only that nothing changed.
    Unexplained,
    AlreadyCancelled,
    AlreadyActive,
    AlreadyOnInterval,
    NotSwitchable,
    NotFound,
}

/// A write to Allodia's own subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Write {
    Checkout(AllodiaPlan),
    Cancel,
    Resubscribe,
    Switch(AllodiaPlan),
}

/// What one read learned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Unavailable,
    NeedsReauth,
    Loaded(Box<AllodiaSubscription>),
}

/// What the section keeps between signals, for one Allodia account.
#[derive(Debug, Default)]
pub(crate) struct SubscriptionWork {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The signed-in account `held` belongs to.
    account: Option<String>,
    held: Held,
    /// The last payment page's id, which keeps rising across accounts so a page opened once is
    /// never opened again.
    pages: u64,
    tickets: u64,
}

#[derive(Debug, Default)]
struct Held {
    answer: Option<Answer>,
    /// The ticket of the answer held: an older read landing later is dropped.
    ticket: u64,
    reading: u32,
    /// From the press until the read that follows it ends.
    writing: Option<Write>,
    outcome: Option<AllodiaSubscriptionOutcome>,
    page: Option<AllodiaCheckoutPage>,
}

/// What [`SubscriptionWork::snapshot`] hands the card.
pub(crate) struct Snapshot {
    pub(crate) answer: Option<Answer>,
    pub(crate) reading: bool,
    pub(crate) writing: Option<Write>,
    pub(crate) outcome: Option<AllodiaSubscriptionOutcome>,
    pub(crate) page: Option<AllodiaCheckoutPage>,
}

impl SubscriptionWork {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .expect("allodia subscription mutex poisoned")
    }

    /// What is held for `account`, the one signed in now: anything held for another goes.
    fn current<'a>(inner: &'a mut Inner, account: &str) -> &'a mut Held {
        if inner.account.as_deref() != Some(account) {
            inner.account = Some(account.to_owned());
            inner.held = Held::default();
        }
        &mut inner.held
    }

    /// What is held for `account`, when that is still the account the section is for: work
    /// landing for an account that has left is dropped.
    fn landing<'a>(inner: &'a mut Inner, account: &str) -> Option<&'a mut Held> {
        (inner.account.as_deref() == Some(account)).then_some(&mut inner.held)
    }

    pub(crate) fn snapshot(&self, account: &str) -> Snapshot {
        let mut inner = self.lock();
        let held = Self::current(&mut inner, account);
        Snapshot {
            answer: held.answer.clone(),
            reading: held.reading > 0,
            writing: held.writing,
            outcome: held.outcome.clone(),
            page: held.page.clone(),
        }
    }

    /// Whether anything has been asked of this account's subscription: an answer held, or a read
    /// running.
    pub(crate) fn asked(&self, account: &str) -> bool {
        let mut inner = self.lock();
        let held = Self::current(&mut inner, account);
        held.answer.is_some() || held.reading > 0
    }

    /// Starts a read the person asked for: its ticket, or `None` while a read or a write runs,
    /// which answers for it.
    pub(crate) fn begin_read(&self, account: &str) -> Option<u64> {
        let mut inner = self.lock();
        let held = Self::current(&mut inner, account);
        if held.reading > 0 || held.writing.is_some() {
            return None;
        }
        Some(inner.ticket())
    }

    /// Starts the read that follows a sign-in or a write, whatever else runs.
    pub(crate) fn begin_reread(&self, account: &str) -> u64 {
        let mut inner = self.lock();
        Self::current(&mut inner, account);
        inner.ticket()
    }

    /// A read landed. Dropped when it is older than the answer held, or the account left.
    pub(crate) fn read(&self, account: &str, ticket: u64, answer: Answer) {
        let mut inner = self.lock();
        let Some(held) = Self::landing(&mut inner, account) else {
            return;
        };
        held.reading = held.reading.saturating_sub(1);
        if ticket > held.ticket {
            held.ticket = ticket;
            held.answer = Some(answer);
        }
    }

    /// Starts `write`; `false` while another runs. The last outcome goes.
    pub(crate) fn begin_write(&self, account: &str, write: Write) -> bool {
        let mut inner = self.lock();
        let held = Self::current(&mut inner, account);
        if held.writing.is_some() {
            return false;
        }
        held.writing = Some(write);
        held.outcome = None;
        true
    }

    /// The service answered a write: what it came to, and a payment page to open.
    pub(crate) fn wrote(
        &self,
        account: &str,
        outcome: Option<AllodiaSubscriptionOutcome>,
        page: Option<String>,
    ) {
        let mut inner = self.lock();
        let id = inner.pages + 1;
        let Some(held) = Self::landing(&mut inner, account) else {
            return;
        };
        held.outcome = outcome;
        if let Some(url) = page {
            held.page = Some(AllodiaCheckoutPage { id, url });
            inner.pages = id;
        }
    }

    /// The read after a write landed: the write ends.
    pub(crate) fn write_ended(&self, account: &str) {
        let mut inner = self.lock();
        if let Some(held) = Self::landing(&mut inner, account) {
            held.writing = None;
        }
    }

    /// Forgets the account's subscription, on sign-out.
    pub(crate) fn forget(&self) {
        let mut inner = self.lock();
        inner.account = None;
        inner.held = Held::default();
    }
}

impl Inner {
    fn ticket(&mut self) -> u64 {
        self.tickets += 1;
        self.held.reading += 1;
        self.tickets
    }
}

#[cfg(test)]
#[path = "allodia_subscription_card_tests.rs"]
mod tests;
