// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What the subscription section shows, decided from one answer (`purchasing.md`, "The account
//! screen"): who is charging, whether anything will charge again, which writes the service
//! permitted and whether one runs. Plain data in and out, so the sentence somebody reads about
//! being charged twice is decided in a unit test.

use crate::{
    AllodiaBiller, AllodiaPlan, AllodiaPurchaseError, AllodiaStore, AllodiaSubscriptionRefusal,
    ControlState,
    allodia_subscription::{
        AllodiaOwnStatus, AllodiaStoreStatus, AllodiaStoreSubscription, AllodiaSubscription,
    },
    allodia_subscription_card::{
        AllodiaCheckoutOffer, AllodiaStorePage, AllodiaSubscriptionView, AllodiaSwitch,
        AllodiaWriteFailure, Write,
    },
};

/// The section for `subscription`, with `writing` running and a read running beside it when
/// `refreshing`.
pub(crate) fn view(
    subscription: &AllodiaSubscription,
    writing: Option<Write>,
    refreshing: bool,
) -> AllodiaSubscriptionView {
    let control = |running: bool| ControlState {
        enabled: writing.is_none(),
        running,
    };
    AllodiaSubscriptionView {
        entitled: subscription.entitled,
        billed_by: billers(subscription).into_iter().next(),
        period_end: subscription.current_period_end.clone(),
        renews: will_renew(subscription),
        grace: in_grace(subscription),
        duplicate_billing: subscription.duplicate_billing.clone(),
        store_pages: store_pages(subscription),
        paid_through: paid_through(subscription),
        switch: switch_target(subscription).map(|to| AllodiaSwitch {
            to,
            control: control(matches!(writing, Some(Write::Switch(_)))),
        }),
        resubscribe: offers_restart(subscription)
            .then(|| control(writing == Some(Write::Resubscribe))),
        cancel: subscription
            .actions
            .can_cancel
            .then(|| control(writing == Some(Write::Cancel))),
        checkout: sells(subscription).then(|| AllodiaCheckoutOffer {
            prices: subscription.prices.clone(),
            monthly: control(writing == Some(Write::Checkout(AllodiaPlan::Monthly))),
            yearly: control(writing == Some(Write::Checkout(AllodiaPlan::Yearly))),
        }),
        refreshing,
    }
}

/// Whether the section offers `write` now: what the dispatch checks before it reaches the
/// service, from the same view the client draws.
pub(crate) fn permits(view: &AllodiaSubscriptionView, write: Write) -> bool {
    let free = |control: Option<&ControlState>| control.is_some_and(|control| control.enabled);
    match write {
        Write::Checkout(AllodiaPlan::Monthly) => free(view.checkout.as_ref().map(|c| &c.monthly)),
        Write::Checkout(AllodiaPlan::Yearly) => free(view.checkout.as_ref().map(|c| &c.yearly)),
        Write::Cancel => free(view.cancel.as_ref()),
        Write::Resubscribe => free(view.resubscribe.as_ref()),
        Write::Switch(plan) => view
            .switch
            .as_ref()
            .is_some_and(|switch| switch.to == plan && switch.control.enabled),
    }
}

/// Everyone charging for this account right now: Allodia's own billing first, then each store
/// once, in the order the service listed them.
///
/// ⚠️ **"Right now" is the whole of it.** The list keeps a subscription after it ends, so an
/// account that bought at one store and later at another carries both, and naming the first would
/// name the dead one.
pub(crate) fn billers(subscription: &AllodiaSubscription) -> Vec<AllodiaBiller> {
    let mut billers = Vec::new();
    if subscription
        .own
        .as_ref()
        .is_some_and(|own| own.status != AllodiaOwnStatus::PendingFirstPayment)
    {
        billers.push(AllodiaBiller::Allodia);
    }
    let mut seen = Vec::new();
    for store in &subscription.stores {
        if !store_is_billing(&store.status) || seen.contains(&store.source) {
            continue;
        }
        seen.push(store.source);
        billers.push(match store.source {
            AllodiaStore::Apple => AllodiaBiller::Apple,
            AllodiaStore::Google => AllodiaBiller::Google,
            AllodiaStore::Allodia => AllodiaBiller::Allodia,
        });
    }
    billers
}

/// Whether somebody is still on a store subscription: renewing, being retried, or cancelled and
/// running out the period already paid for. A status this build does not know is **never read as
/// permission**.
fn store_is_billing(status: &AllodiaStoreStatus) -> bool {
    matches!(
        status,
        AllodiaStoreStatus::Active | AllodiaStoreStatus::Grace | AllodiaStoreStatus::Cancelled
    )
}

/// Whether the store still has something for this person to do about a subscription.
///
/// ⚠️ **Not who is billing them.** On hold and paused grant nothing, so neither claims "billed
/// by", and both are fixed only at the store, so their page stays. Expired and revoked leave
/// nothing to manage, and offering the store's page then walks somebody into its resubscribe
/// button while another source may already charge them.
fn store_can_be_managed(status: &AllodiaStoreStatus) -> bool {
    store_is_billing(status)
        || matches!(
            status,
            AllodiaStoreStatus::OnHold | AllodiaStoreStatus::Paused
        )
}

/// One page per store rather than per subscription: an account carries two at one store the
/// moment somebody resubscribes there, and the store's page is the store's.
fn store_pages(subscription: &AllodiaSubscription) -> Vec<AllodiaStorePage> {
    let mut pages: Vec<AllodiaStorePage> = Vec::new();
    for store in subscription
        .stores
        .iter()
        .filter(|store| store_can_be_managed(&store.status))
    {
        if !pages.iter().any(|page| page.store == store.source) {
            pages.push(page(store));
        }
    }
    pages
}

fn page(store: &AllodiaStoreSubscription) -> AllodiaStorePage {
    AllodiaStorePage {
        store: store.source,
        url: store.manage_url.clone(),
    }
}

/// Whether anything will charge again. A store subscription says so itself; Allodia's own by
/// still having a next payment. Not a reading of `entitled`, which answers a different question.
fn will_renew(subscription: &AllodiaSubscription) -> bool {
    subscription.stores.iter().any(|store| store.auto_renewing)
        || subscription.own.as_ref().is_some_and(|own| {
            own.status == AllodiaOwnStatus::Active
                && own
                    .next_payment_date
                    .as_deref()
                    .is_some_and(|date| !date.is_empty())
        })
}

fn in_grace(subscription: &AllodiaSubscription) -> bool {
    subscription
        .stores
        .iter()
        .any(|store| store.status == AllodiaStoreStatus::Grace)
        || subscription
            .own
            .as_ref()
            .is_some_and(|own| own.status == AllodiaOwnStatus::PastDue)
}

/// The other period, when the service permits a switch of Allodia's own subscription.
fn switch_target(subscription: &AllodiaSubscription) -> Option<AllodiaPlan> {
    if !subscription.actions.can_switch_interval {
        return None;
    }
    match subscription.own.as_ref()?.interval {
        AllodiaPlan::Monthly => Some(AllodiaPlan::Yearly),
        AllodiaPlan::Yearly => Some(AllodiaPlan::Monthly),
    }
}

/// A restart is offered for a cancelled subscription the service says can restart: one whose
/// period has run out is a new checkout instead, and only the service knows which.
fn offers_restart(subscription: &AllodiaSubscription) -> bool {
    subscription.actions.can_resubscribe
        && subscription
            .own
            .as_ref()
            .is_some_and(|own| own.status == AllodiaOwnStatus::Cancelled)
}

/// Allodia's checkout is offered while nothing is charging and this deployment has billing.
/// `can_start_checkout` is false for anybody already paying through any source.
fn sells(subscription: &AllodiaSubscription) -> bool {
    !subscription.entitled
        && subscription.checkout_available
        && subscription.actions.can_start_checkout
}

fn paid_through(subscription: &AllodiaSubscription) -> Option<String> {
    subscription
        .own
        .as_ref()
        .and_then(|own| own.current_period_end.clone())
        .or_else(|| subscription.current_period_end.clone())
}

/// What a failed write can say. Everything that is not a refusal is
/// [`AllodiaWriteFailure::Unexplained`], an outage included: nothing was learned.
pub(crate) fn write_failure(error: &AllodiaPurchaseError) -> AllodiaWriteFailure {
    let AllodiaPurchaseError::Refused { reason } = error else {
        return AllodiaWriteFailure::Unexplained;
    };
    match reason {
        AllodiaSubscriptionRefusal::AlreadyCancelled => AllodiaWriteFailure::AlreadyCancelled,
        AllodiaSubscriptionRefusal::AlreadyActive => AllodiaWriteFailure::AlreadyActive,
        AllodiaSubscriptionRefusal::AlreadyOnInterval => AllodiaWriteFailure::AlreadyOnInterval,
        AllodiaSubscriptionRefusal::NotSwitchable => AllodiaWriteFailure::NotSwitchable,
        AllodiaSubscriptionRefusal::NotFound => AllodiaWriteFailure::NotFound,
        AllodiaSubscriptionRefusal::Unavailable | AllodiaSubscriptionRefusal::Other => {
            AllodiaWriteFailure::Unexplained
        }
    }
}

#[cfg(test)]
#[path = "allodia_subscription_facts_tests.rs"]
mod tests;
