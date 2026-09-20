//! The subscription section's pure decisions, which the `purchasing.md` contract beside the
//! Allodia Licence binds every client to. Each of these fails *plausibly* rather than loudly, and
//! every one of them is about money:
//!
//! * read a store status by exclusion and an unknown one grants access it never said it had;
//! * name every store the service listed and a device billed by Google Play is told it is billed by
//!   Apple, whose subscription ran out hours earlier;
//! * draw one manage button per subscription and a resubscribed account gets two identical ones;
//! * let a refusal reach the screen as anything but its own sentence and somebody reads a code.

use mailcal_bindings::{
    AllodiaBiller, AllodiaOwnStatus, AllodiaOwnSubscription, AllodiaPlan, AllodiaPrices,
    AllodiaPurchaseError, AllodiaStore, AllodiaStoreStatus, AllodiaStoreSubscription,
    AllodiaSubscription, AllodiaSubscriptionActions, AllodiaSubscriptionRefusal,
};

use super::{
    WriteFailure, biller_name, billers, in_grace, manageable_stores, minor_units, offers_restart,
    store_can_be_managed, store_is_billing, switch_target, will_renew, write_failure,
};

fn actions() -> AllodiaSubscriptionActions {
    AllodiaSubscriptionActions {
        can_cancel: false,
        can_resubscribe: false,
        can_switch_interval: false,
        can_start_checkout: false,
    }
}

fn subscription(
    own: Option<AllodiaOwnSubscription>,
    stores: Vec<AllodiaStoreSubscription>,
) -> AllodiaSubscription {
    AllodiaSubscription {
        entitled: true,
        plan: "paid".to_owned(),
        current_period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        own,
        stores,
        duplicate_billing: Vec::new(),
        actions: actions(),
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
        amount_in_cents: 499,
        next_payment_date: next_payment.map(str::to_owned),
        current_period_end: Some("2026-11-01T00:00:00+00:00".to_owned()),
        cancelled_at: None,
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
        interval: AllodiaPlan::Monthly,
        product_id: None,
        price_in_cents: None,
        currency: None,
        current_period_end: None,
        auto_renewing,
        environment: "production".to_owned(),
        manage_url: "https://example.test/manage".to_owned(),
    }
}

/// ⚠️ A status this build has never heard of is never read as permission. Reading the enum by
/// exclusion ("everything but expired and revoked") is what would grant on one.
#[test]
fn a_status_this_build_cannot_name_grants_nothing_and_offers_nothing() {
    let unknown = AllodiaStoreStatus::Unknown {
        label: "on_probation".to_owned(),
    };
    assert!(!store_is_billing(&unknown));
    assert!(!store_can_be_managed(&unknown));
}

/// "Who is billing you" and "is there anything to do at the store" are different questions, and
/// they part company on exactly the two states where somebody most needs the answer.
#[test]
fn billing_and_manageable_differ_on_the_states_that_strand_somebody() {
    // On hold and paused grant nothing, so neither may claim the "billed by" line, and both are
    // fixed only at the store: dropping their button strands the person it matters to.
    for status in [AllodiaStoreStatus::OnHold, AllodiaStoreStatus::Paused] {
        assert!(!store_is_billing(&status), "{status:?}");
        assert!(store_can_be_managed(&status), "{status:?}");
    }
    // The other way about: nothing is left to manage, and offering the route anyway walks somebody
    // into the store's own resubscribe button while another source is already charging them.
    for status in [AllodiaStoreStatus::Expired, AllodiaStoreStatus::Revoked] {
        assert!(!store_is_billing(&status), "{status:?}");
        assert!(!store_can_be_managed(&status), "{status:?}");
    }
}

/// ⚠️ The store list keeps a subscription after it ends, so an account that bought at one store and
/// later at another carries both. Naming the first named the dead one.
#[test]
fn only_a_store_still_charging_claims_the_billed_by_line() {
    let answer = subscription(
        None,
        vec![
            store(AllodiaStore::Apple, AllodiaStoreStatus::Expired, false),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    let named = billers(&answer);
    assert_eq!(named.len(), 1, "{named:?}");
    assert_eq!(biller_name(&named[0]), "Google Play");
}

/// ⚠️ Two subscriptions at one store get one way in, not two. Resubscribing is what produces the
/// pair: the lapsed one is still inside the period it was paid for, so both are manageable.
#[test]
fn two_subscriptions_at_one_store_get_one_way_in() {
    let both = subscription(
        None,
        vec![
            store(AllodiaStore::Google, AllodiaStoreStatus::Cancelled, false),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    assert_eq!(manageable_stores(&both).len(), 1);
    assert_eq!(billers(&both).len(), 1);

    // One per store, not one in total.
    let apart = subscription(
        None,
        vec![
            store(AllodiaStore::Apple, AllodiaStoreStatus::Active, true),
            store(AllodiaStore::Google, AllodiaStoreStatus::Active, true),
        ],
    );
    assert_eq!(manageable_stores(&apart).len(), 2);
    assert_eq!(billers(&apart).len(), 2);
}

/// A checkout that has been started and never paid is not somebody being billed.
#[test]
fn an_unpaid_checkout_is_nobody_billing_you() {
    let pending = subscription(
        Some(own(AllodiaOwnStatus::PendingFirstPayment, None)),
        Vec::new(),
    );
    assert!(billers(&pending).is_empty());
}

/// "Renews on" against "runs until" is the difference between a charge somebody expects and one
/// they do not. A cancelled subscription still has a period end; what it has lost is the next
/// payment.
#[test]
fn renewal_is_the_next_payment_and_not_the_period_end() {
    let renewing = subscription(
        Some(own(AllodiaOwnStatus::Active, Some("2026-11-01T00:00:00Z"))),
        Vec::new(),
    );
    assert!(will_renew(&renewing));
    let cancelled = subscription(Some(own(AllodiaOwnStatus::Cancelled, None)), Vec::new());
    assert!(!will_renew(&cancelled));
    // An active own subscription with no next payment is one nothing will charge again either.
    assert!(!will_renew(&subscription(
        Some(own(AllodiaOwnStatus::Active, None)),
        Vec::new()
    )));
}

/// A failed charge being retried is not a lapse, and access continues through it.
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

/// `actions` decides, never the client. The service computed it with every biller in view, which
/// is a thing no client is in a position to do.
#[test]
fn no_write_is_offered_that_the_service_has_not_permitted() {
    let forbidden = subscription(Some(own(AllodiaOwnStatus::Cancelled, None)), Vec::new());
    assert!(switch_target(&forbidden).is_none());
    assert!(!offers_restart(&forbidden));

    let mut permitted = forbidden;
    permitted.actions.can_switch_interval = true;
    permitted.actions.can_resubscribe = true;
    assert_eq!(switch_target(&permitted), Some(AllodiaPlan::Yearly));
    assert!(offers_restart(&permitted));
}

/// A restart is offered on a cancelled subscription alone: one whose period has run out is a new
/// checkout rather than a restart, and only the service knows which.
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

/// ⚠️ A refusal is a code and the sentence is the client's, which is what `purchasing.md` asks for:
/// "you have already cancelled" and "that cannot change while a charge is being retried" are
/// different things to say, and only the code tells them apart.
#[test]
fn each_refusal_is_its_own_answer_and_everything_else_is_unexplained() {
    let refused = |reason| AllodiaPurchaseError::Refused { reason };
    assert_eq!(
        write_failure(&refused(AllodiaSubscriptionRefusal::NotSwitchable)),
        WriteFailure::NotSwitchable
    );
    assert_eq!(
        write_failure(&refused(AllodiaSubscriptionRefusal::AlreadyCancelled)),
        WriteFailure::AlreadyCancelled
    );
    // Billing not configured on this deployment names nothing a client could truthfully say.
    assert_eq!(
        write_failure(&refused(AllodiaSubscriptionRefusal::Unavailable)),
        WriteFailure::Unexplained
    );
    // An outage learned nothing, so there is nothing to explain there either.
    assert_eq!(
        write_failure(&AllodiaPurchaseError::Unreachable),
        WriteFailure::Unexplained
    );
}

/// Minor units and an ISO code, formatted by this side because the core carries no locale data.
/// The code rather than a symbol: this client carries no currency data, and a borrowed symbol
/// prices an amount in the wrong money while looking entirely correct.
#[test]
fn an_amount_is_priced_in_the_readers_own_punctuation_and_named_outright() {
    assert_eq!(minor_units(499, "EUR", "nl"), "4,99 EUR");
    assert_eq!(minor_units(499, "EUR", "en"), "4.99 EUR");
    // The hundredths keep both digits, or 4,90 is drawn as 4,9 and reads as a different price.
    assert_eq!(minor_units(490, "EUR", "nl"), "4,90 EUR");
    assert_eq!(minor_units(4900, "EUR", "nl"), "49,00 EUR");
    // A currency the service did not name is still the right amount.
    assert_eq!(minor_units(499, "", "nl"), "4,99");
}

/// A biller this build cannot name still has to appear, or a "you are being charged twice" warning
/// would name only one of the two.
#[test]
fn a_biller_this_build_cannot_name_is_still_named() {
    assert_eq!(biller_name(&AllodiaBiller::Allodia), "Allodia");
    assert_eq!(
        biller_name(&AllodiaBiller::Unknown {
            label: "A Shop".to_owned()
        }),
        "A Shop"
    );
}
