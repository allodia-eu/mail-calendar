// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The subscription section's decisions, which the `purchasing.md` contract binds every client
//! to. Each fails *plausibly* rather than loudly, and every one is about money:
//!
//! * read a store status by exclusion and an unknown one grants access it never said it had;
//! * name every store the service listed and a device billed by Google Play is told it is billed by
//!   Apple, whose subscription ran out hours earlier;
//! * draw one store page per subscription and a resubscribed account gets two identical ones;
//! * offer a write the service did not permit and its only outcome is a refusal.

use super::{
    billers, in_grace, offers_restart, permits, store_can_be_managed, store_is_billing,
    switch_target, view, will_renew, write_failure,
};
use crate::{
    AllodiaBiller, AllodiaPlan, AllodiaPrices, AllodiaPurchaseError, AllodiaStore,
    AllodiaSubscriptionRefusal, AllodiaWriteFailure, ControlState,
    allodia_subscription::{
        AllodiaOwnStatus, AllodiaOwnSubscription, AllodiaStoreStatus, AllodiaStoreSubscription,
        AllodiaSubscription, AllodiaSubscriptionActions,
    },
    allodia_subscription_card::Write,
};

fn subscription(
    own: Option<AllodiaOwnSubscription>,
    stores: Vec<AllodiaStoreSubscription>,
) -> AllodiaSubscription {
    AllodiaSubscription {
        entitled: true,
        current_period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        own,
        stores,
        duplicate_billing: Vec::new(),
        actions: AllodiaSubscriptionActions {
            can_cancel: false,
            can_resubscribe: false,
            can_switch_interval: false,
            can_start_checkout: false,
        },
        prices: AllodiaPrices {
            monthly_in_cents: 499,
            yearly_in_cents: 4900,
            currency: "EUR".to_owned(),
        },
        checkout_available: true,
    }
}

fn own(status: AllodiaOwnStatus, next_payment: Option<&str>) -> AllodiaOwnSubscription {
    AllodiaOwnSubscription {
        status,
        interval: AllodiaPlan::Monthly,
        next_payment_date: next_payment.map(str::to_owned),
        current_period_end: Some("2026-10-01T00:00:00+00:00".to_owned()),
    }
}

fn store(
    source: AllodiaStore,
    status: AllodiaStoreStatus,
    auto_renewing: bool,
) -> AllodiaStoreSubscription {
    AllodiaStoreSubscription {
        source,
        status,
        auto_renewing,
        manage_url: format!("https://example.test/{source:?}"),
    }
}

const FREE: ControlState = ControlState {
    enabled: true,
    running: false,
};

/// ⚠️ A status this build has never heard of is never read as permission.
#[test]
fn a_status_this_build_cannot_name_grants_nothing_and_offers_nothing() {
    assert!(!store_is_billing(&AllodiaStoreStatus::Unknown));
    assert!(!store_can_be_managed(&AllodiaStoreStatus::Unknown));
}

/// "Who is billing you" and "is there anything to do at the store" part company on exactly the
/// states where somebody most needs the answer.
#[test]
fn billing_and_manageable_differ_on_the_states_that_strand_somebody() {
    for status in [AllodiaStoreStatus::OnHold, AllodiaStoreStatus::Paused] {
        assert!(!store_is_billing(&status), "{status:?}");
        assert!(store_can_be_managed(&status), "{status:?}");
    }
    for status in [AllodiaStoreStatus::Expired, AllodiaStoreStatus::Revoked] {
        assert!(!store_is_billing(&status), "{status:?}");
        assert!(!store_can_be_managed(&status), "{status:?}");
    }
}

/// ⚠️ The store list keeps a subscription after it ends: naming the first named the dead one.
#[test]
fn only_a_store_still_charging_is_named_as_billing() {
    let answer = subscription(
        None,
        vec![
            store(AllodiaStore::Apple, AllodiaStoreStatus::Expired, false),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    assert_eq!(billers(&answer), [AllodiaBiller::Google]);
    let drawn = view(&answer, None, false);
    assert_eq!(drawn.billed_by, Some(AllodiaBiller::Google));
    assert_eq!(drawn.store_pages.len(), 1);
    assert_eq!(drawn.store_pages[0].store, AllodiaStore::Google);
}

/// ⚠️ Two subscriptions at one store get one page, not two; two stores get one each.
#[test]
fn two_subscriptions_at_one_store_get_one_page() {
    let both = subscription(
        None,
        vec![
            store(AllodiaStore::Google, AllodiaStoreStatus::Cancelled, false),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    assert_eq!(view(&both, None, false).store_pages.len(), 1);
    assert_eq!(billers(&both).len(), 1);

    let apart = subscription(
        None,
        vec![
            store(AllodiaStore::Apple, AllodiaStoreStatus::Active, true),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    assert_eq!(view(&apart, None, false).store_pages.len(), 2);
    assert_eq!(billers(&apart).len(), 2);
}

/// A checkout started and never paid is nobody billing you, and Allodia's own billing is named
/// before any store.
#[test]
fn allodia_is_named_first_and_an_unpaid_checkout_not_at_all() {
    let pending = subscription(
        Some(own(AllodiaOwnStatus::PendingFirstPayment, None)),
        Vec::new(),
    );
    assert_eq!(billers(&pending), []);

    let twice = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01"))),
        vec![store(AllodiaStore::Apple, AllodiaStoreStatus::Active, true)],
    );
    assert_eq!(
        billers(&twice),
        [AllodiaBiller::Allodia, AllodiaBiller::Apple]
    );
}

/// "Renews on" against "runs until" is the next payment, not the period end.
#[test]
fn renewal_is_the_next_payment_and_not_the_period_end() {
    let renewing = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01T00:00:00Z"))),
        Vec::new(),
    );
    assert!(will_renew(&renewing));
    let cancelled = subscription(Some(own(AllodiaOwnStatus::Cancelled, None)), Vec::new());
    assert!(!will_renew(&cancelled));
    assert!(!will_renew(&subscription(
        Some(own(AllodiaOwnStatus::Active, None)),
        Vec::new()
    )));
    // An empty date is no next payment either.
    assert!(!will_renew(&subscription(
        Some(own(AllodiaOwnStatus::Active, Some(""))),
        Vec::new()
    )));
}

/// A failed charge being retried is grace, on both routes.
#[test]
fn a_retried_charge_is_grace_on_both_routes() {
    assert!(in_grace(&subscription(
        Some(own(AllodiaOwnStatus::PastDue, None)),
        Vec::new()
    )));
    assert!(in_grace(&subscription(
        None,
        vec![store(AllodiaStore::Apple, AllodiaStoreStatus::Grace, true)]
    )));
    assert!(!in_grace(&subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01T00:00:00Z"))),
        Vec::new()
    )));
}

/// `actions` decides which writes exist: the service computed it with every biller in view.
#[test]
fn no_write_is_offered_that_the_service_has_not_permitted() {
    let forbidden = subscription(Some(own(AllodiaOwnStatus::Cancelled, None)), Vec::new());
    let drawn = view(&forbidden, None, false);
    assert!(drawn.switch.is_none() && drawn.resubscribe.is_none() && drawn.cancel.is_none());
    for write in [
        Write::Cancel,
        Write::Resubscribe,
        Write::Switch(AllodiaPlan::Yearly),
    ] {
        assert!(!permits(&drawn, write), "{write:?}");
    }

    let mut permitted = forbidden;
    permitted.actions.can_switch_interval = true;
    permitted.actions.can_resubscribe = true;
    permitted.actions.can_cancel = true;
    assert_eq!(switch_target(&permitted), Some(AllodiaPlan::Yearly));
    let drawn = view(&permitted, None, false);
    assert_eq!(drawn.resubscribe, Some(FREE));
    assert_eq!(drawn.cancel, Some(FREE));
    assert!(permits(&drawn, Write::Switch(AllodiaPlan::Yearly)));
    // The switch goes to the other period, never the one already held.
    assert!(!permits(&drawn, Write::Switch(AllodiaPlan::Monthly)));
}

/// A restart is offered on a cancelled subscription of Allodia's own alone.
#[test]
fn a_restart_is_offered_on_a_cancelled_subscription_alone() {
    let mut active = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01T00:00:00Z"))),
        Vec::new(),
    );
    active.actions.can_resubscribe = true;
    assert!(!offers_restart(&active));

    let mut none_of_ours = subscription(None, Vec::new());
    none_of_ours.actions.can_resubscribe = true;
    assert!(!offers_restart(&none_of_ours));
}

/// Allodia's checkout is offered only while nothing charges, the deployment bills, and the
/// service permits it.
#[test]
fn the_checkout_is_offered_only_to_somebody_nobody_charges() {
    let mut free = subscription(None, Vec::new());
    free.entitled = false;
    free.actions.can_start_checkout = true;
    let offer = view(&free, None, false).checkout.expect("offered");
    assert_eq!(offer.prices.currency, "EUR");
    assert_eq!((offer.monthly, offer.yearly), (FREE, FREE));

    let mut no_billing = free.clone();
    no_billing.checkout_available = false;
    assert!(view(&no_billing, None, false).checkout.is_none());

    let mut refused = free.clone();
    refused.actions.can_start_checkout = false;
    assert!(view(&refused, None, false).checkout.is_none());

    let mut paying = free;
    paying.entitled = true;
    assert!(view(&paying, None, false).checkout.is_none());
}

/// A write runs on its own control, and holds every other until the read after it lands.
#[test]
fn a_running_write_holds_every_control() {
    let mut answer = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01"))),
        Vec::new(),
    );
    answer.actions.can_cancel = true;
    answer.actions.can_switch_interval = true;
    let drawn = view(&answer, Some(Write::Cancel), false);
    let cancel = drawn.cancel.expect("offered");
    assert!(cancel.running && !cancel.enabled);
    let switch = drawn.switch.clone().expect("offered").control;
    assert!(!switch.running && !switch.enabled);
    assert!(!permits(&drawn, Write::Switch(AllodiaPlan::Yearly)));
}

/// Both confirmations name what has been paid for: Allodia's own period first.
#[test]
fn paid_through_is_the_own_period_before_the_furthest() {
    let answer = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01"))),
        Vec::new(),
    );
    assert_eq!(
        view(&answer, None, false).paid_through.as_deref(),
        Some("2026-10-01T00:00:00+00:00")
    );
    assert_eq!(
        view(&subscription(None, Vec::new()), None, false)
            .paid_through
            .as_deref(),
        Some("2026-11-01T00:00:00+00:00")
    );
}

/// ⚠️ A refusal is a code and the sentence is the client's; anything not a refusal is
/// unexplained.
#[test]
fn each_refusal_is_its_own_answer_and_everything_else_is_unexplained() {
    let refused = |reason| AllodiaPurchaseError::Refused { reason };
    assert_eq!(
        write_failure(&refused(AllodiaSubscriptionRefusal::NotSwitchable)),
        AllodiaWriteFailure::NotSwitchable
    );
    assert_eq!(
        write_failure(&refused(AllodiaSubscriptionRefusal::AlreadyCancelled)),
        AllodiaWriteFailure::AlreadyCancelled
    );
    for nothing_to_say in [
        refused(AllodiaSubscriptionRefusal::Unavailable),
        refused(AllodiaSubscriptionRefusal::Other),
        AllodiaPurchaseError::Unreachable,
        AllodiaPurchaseError::SignedOut,
    ] {
        assert_eq!(
            write_failure(&nothing_to_say),
            AllodiaWriteFailure::Unexplained
        );
    }
}

/// Allodia's own subscription in a status this build cannot name is still named as billing, so a
/// duplicate charge is never hidden, but it is never said to renew.
#[test]
fn an_own_status_this_build_cannot_name_is_named_and_never_renews() {
    let unknown = subscription(
        Some(own(AllodiaOwnStatus::Unknown, Some("2026-11-01"))),
        Vec::new(),
    );
    assert_eq!(billers(&unknown), [AllodiaBiller::Allodia]);
    assert!(!will_renew(&unknown));
    assert!(!in_grace(&unknown));
}

/// ⚠️ On hold and paused grant nothing, so the section draws its free half, and the store's page
/// is still offered there: a failed card is replaced at the store and a pause lifted there.
#[test]
fn a_store_page_is_offered_to_somebody_the_store_holds_without_access() {
    let mut held = subscription(
        None,
        vec![store(
            AllodiaStore::Google,
            AllodiaStoreStatus::OnHold,
            true,
        )],
    );
    held.entitled = false;
    let drawn = view(&held, None, false);
    assert!(!drawn.entitled);
    assert_eq!(drawn.billed_by, None);
    assert_eq!(drawn.store_pages.len(), 1);
}
