//! What the subscription section says, as decisions a test can fail.
//!
//! The rules are `purchasing.md`, the contract that ships beside the Allodia Licence, and the core
//! holds every one of them. What is here is the reading of one answer: who is charging, whether
//! anything will charge again, which buttons the service permitted, and how a date and an amount
//! are put in front of a reader. Its Apple, Android and Windows twins are
//! `AllodiaSubscriptionSettings.swift`, `AllodiaSubscriptionModel.kt` and
//! `AllodiaSubscriptionFacts.cs`: keep the states and the wording in step.
//!
//! ⚠️ **None of this gates a capability.** The entitlement does, locally and without a network
//! call. This is what the screen *says*, so a service that cannot be reached costs somebody a
//! sentence, never access they have paid for.
//!
//! Every function here is plain data in and plain data out, so the sentence somebody reads about
//! being charged twice is decided in a unit test rather than by building a window.

use mailcal_bindings::{
    AllodiaBiller, AllodiaOwnStatus, AllodiaPlan, AllodiaPurchaseError, AllodiaStore,
    AllodiaStoreStatus, AllodiaStoreSubscription, AllodiaSubscription, AllodiaSubscriptionRefusal,
};

use crate::l10n;

/// Why a write did not happen, as the sentence to put in front of the person.
///
/// ⚠️ **A code, never a message from anywhere else.** The core reports a refusal as a
/// [`AllodiaSubscriptionRefusal`], and `purchasing.md` asks a client to switch on it, because "you
/// have already cancelled" and "that cannot change while a charge is being retried" are different
/// things to say and only the code tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriteFailure {
    /// Nothing more can be said: an outage, a refused token, a reason this build has never heard
    /// of. The sentence says only that nothing changed.
    Unexplained,
    AlreadyCancelled,
    AlreadyActive,
    AlreadyOnInterval,
    NotSwitchable,
    NotFound,
}

/// What a biller is called on screen.
///
/// **Allodia, never the payment processor behind it.** Naming a processor somebody has never heard
/// of, inside a message about being charged twice, is how a correct warning reads as a scam. The
/// bare company name is right here, where the sentence is about who is taking the money rather
/// than about the app.
pub(crate) fn biller_name(biller: &AllodiaBiller) -> String {
    match biller {
        AllodiaBiller::Allodia => "Allodia".to_owned(),
        AllodiaBiller::Apple => "Apple".to_owned(),
        AllodiaBiller::Google => "Google Play".to_owned(),
        // A biller this build cannot name still has to appear, or a "you are being charged twice"
        // warning would name only one of the two.
        AllodiaBiller::Unknown { label } => label.clone(),
    }
}

/// Everyone charging for this account right now, in a stable order: Allodia's own billing first,
/// then each store in the order the service listed them.
///
/// ⚠️ **"Right now" is the whole of it, and reading every store the service listed was a bug.**
/// The list keeps a subscription after it ends, so an account that bought at one store and later
/// at another carries both, and naming the first named the dead one.
///
/// Whether a store still has something to manage is a different question, answered by
/// [`store_can_be_managed`], and the two part company on the states that matter most.
pub(crate) fn billers(subscription: &AllodiaSubscription) -> Vec<AllodiaBiller> {
    let mut billers = Vec::new();
    if subscription
        .own
        .as_ref()
        .is_some_and(|own| own.status != AllodiaOwnStatus::PendingFirstPayment)
    {
        billers.push(AllodiaBiller::Allodia);
    }
    // Distinct for the same reason the manage buttons are: two subscriptions at one store is an
    // ordinary shape, and naming that store twice would say somebody is charged twice by it.
    let mut seen = Vec::new();
    for store in &subscription.stores {
        if !store_is_billing(&store.status) || seen.contains(&store.source) {
            continue;
        }
        seen.push(store.source);
        billers.push(match store.source {
            AllodiaStore::Apple => AllodiaBiller::Apple,
            AllodiaStore::Google => AllodiaBiller::Google,
            // The service never reports its own billing as a store, so this arm is unreachable
            // rather than meaningful. Saying "Allodia" is still the right answer if it ever is.
            AllodiaStore::Allodia => AllodiaBiller::Allodia,
        });
    }
    billers
}

/// Whether a store subscription is one somebody is still on: renewing, being retried, or cancelled
/// and running out the period already paid for.
///
/// The permitting states are listed and everything else is false, so a status this build does not
/// know is **never read as permission**, which is the rule the core states about the same enum.
pub(crate) fn store_is_billing(status: &AllodiaStoreStatus) -> bool {
    matches!(
        status,
        AllodiaStoreStatus::Active | AllodiaStoreStatus::Grace | AllodiaStoreStatus::Cancelled
    )
}

/// Whether the store still has something for this person to do about this subscription.
///
/// ⚠️ **Not the same question as who is billing them, and the two answers differ on exactly the
/// states somebody needs most.** On hold and paused grant nothing, so neither may claim the
/// "billed by" line, and both are fixed only at the store: a card that failed is replaced there
/// and a pause is lifted there. Dropping their button strands the person it matters to.
///
/// Expired and revoked are the other way about. Nothing is left to manage, and offering the route
/// anyway walks somebody into the store's own resubscribe button while another source is already
/// charging them, which is the duplicate billing this contract warns about rather than causes.
pub(crate) fn store_can_be_managed(status: &AllodiaStoreStatus) -> bool {
    matches!(
        status,
        AllodiaStoreStatus::Active
            | AllodiaStoreStatus::Grace
            | AllodiaStoreStatus::Cancelled
            | AllodiaStoreStatus::OnHold
            | AllodiaStoreStatus::Paused
    )
}

/// The stores worth offering a way into, one entry per store rather than per subscription.
///
/// ⚠️ **An account can carry more than one subscription at the same store**, and it does the moment
/// somebody resubscribes: the lapsed one is still inside the period it was paid for, so both are
/// manageable and both were drawn, as two identical buttons opening the same page. A store's
/// subscription page is the store's, not the subscription's, so one button is the whole of what
/// there is to offer.
pub(crate) fn manageable_stores(
    subscription: &AllodiaSubscription,
) -> Vec<&AllodiaStoreSubscription> {
    let mut seen = Vec::new();
    let mut stores = Vec::new();
    for store in &subscription.stores {
        if !store_can_be_managed(&store.status) || seen.contains(&store.source) {
            continue;
        }
        seen.push(store.source);
        stores.push(store);
    }
    stores
}

/// Whether anything will charge again, which decides between "renews on" and "runs until".
///
/// A store subscription says so itself; Allodia's own says so by still having a next payment.
/// Deliberately not a reading of `entitled`, which answers a different question.
pub(crate) fn will_renew(subscription: &AllodiaSubscription) -> bool {
    subscription.stores.iter().any(|store| store.auto_renewing)
        || subscription.own.as_ref().is_some_and(|own| {
            own.status == AllodiaOwnStatus::Active
                && own
                    .next_payment_date
                    .as_deref()
                    .is_some_and(|date| !date.is_empty())
        })
}

/// Whether a charge has failed and is being retried. **Access continues**, so this is never drawn
/// as a lapse.
pub(crate) fn in_grace(subscription: &AllodiaSubscription) -> bool {
    subscription
        .stores
        .iter()
        .any(|store| store.status == AllodiaStoreStatus::Grace)
        || subscription
            .own
            .as_ref()
            .is_some_and(|own| own.status == AllodiaOwnStatus::PastDue)
}

/// The period a switch would move to, which is simply the other one.
///
/// `None` when there is nothing to switch: no subscription of Allodia's own, or a service that has
/// said this account may not switch.
pub(crate) fn switch_target(subscription: &AllodiaSubscription) -> Option<AllodiaPlan> {
    if !subscription.actions.can_switch_interval {
        return None;
    }
    match subscription.own.as_ref()?.interval {
        AllodiaPlan::Monthly => Some(AllodiaPlan::Yearly),
        AllodiaPlan::Yearly => Some(AllodiaPlan::Monthly),
    }
}

/// Whether to offer starting the subscription again.
///
/// A different question from whether it has been cancelled: the service decides, because a
/// cancelled subscription whose period has run out is a new checkout rather than a restart, and
/// only it knows which.
pub(crate) fn offers_restart(subscription: &AllodiaSubscription) -> bool {
    subscription.actions.can_resubscribe
        && subscription
            .own
            .as_ref()
            .is_some_and(|own| own.status == AllodiaOwnStatus::Cancelled)
}

/// The day both confirmations talk about: what has already been paid for.
pub(crate) fn paid_through(subscription: &AllodiaSubscription) -> Option<&str> {
    subscription
        .own
        .as_ref()
        .and_then(|own| own.current_period_end.as_deref())
        .or(subscription.current_period_end.as_deref())
}

/// The sentence a failed write earns.
///
/// Everything that is not a refusal is [`WriteFailure::Unexplained`], including a service that
/// could not be reached: nothing was learned, so there is nothing to explain, and the one thing
/// worth saying is that nothing changed.
pub(crate) fn write_failure(error: &AllodiaPurchaseError) -> WriteFailure {
    let AllodiaPurchaseError::Refused { reason } = error else {
        return WriteFailure::Unexplained;
    };
    match reason {
        AllodiaSubscriptionRefusal::AlreadyCancelled => WriteFailure::AlreadyCancelled,
        AllodiaSubscriptionRefusal::AlreadyActive => WriteFailure::AlreadyActive,
        AllodiaSubscriptionRefusal::AlreadyOnInterval => WriteFailure::AlreadyOnInterval,
        AllodiaSubscriptionRefusal::NotSwitchable => WriteFailure::NotSwitchable,
        AllodiaSubscriptionRefusal::NotFound => WriteFailure::NotFound,
        // Unavailable (no billing on this deployment), and a reason a later service invents, both
        // leave a client with nothing specific it could truthfully say.
        _ => WriteFailure::Unexplained,
    }
}

/// A refusal's own words.
pub(crate) fn write_failure_text(failure: WriteFailure) -> &'static str {
    match failure {
        WriteFailure::AlreadyCancelled => l10n::settings_subscription_refused_already_cancelled(),
        WriteFailure::AlreadyActive => l10n::settings_subscription_refused_already_active(),
        WriteFailure::AlreadyOnInterval => {
            l10n::settings_subscription_refused_already_on_interval()
        }
        WriteFailure::NotSwitchable => l10n::settings_subscription_refused_not_switchable(),
        WriteFailure::NotFound => l10n::settings_subscription_refused_not_found(),
        WriteFailure::Unexplained => l10n::settings_subscription_write_failed(),
    }
}

/// Minor units and an ISO 4217 code as something a reader can price.
///
/// ⚠️ **Only for what Allodia bills directly.** A store's price is the store's own formatted
/// string, drawn and never recomputed; this is the other route, where the service sends 199 and
/// `EUR` because the core carries no locale data at all.
///
/// The code rather than a symbol, in the order most of Europe writes one. This client carries no
/// currency data and will not grow a hand-kept symbol table: a borrowed symbol prices an amount in
/// the wrong money while looking entirely correct, where a code is merely plainer.
pub(crate) fn minor_units(minor: i64, currency: &str, locale: &str) -> String {
    let negative = minor < 0;
    let units = minor.unsigned_abs() / 100;
    let hundredths = minor.unsigned_abs() % 100;
    let amount = format!(
        "{}{units}{}{hundredths:02}",
        if negative { "-" } else { "" },
        decimal_separator(locale)
    );
    if currency.is_empty() {
        amount
    } else {
        format!("{amount} {currency}")
    }
}

/// What this language puts between the units and the hundredths. English is the one the catalog
/// ships that writes a point.
pub(crate) fn decimal_separator(locale: &str) -> char {
    if locale == "en" { '.' } else { ',' }
}

#[cfg(test)]
#[path = "allodia_subscription_facts_tests.rs"]
mod tests;
