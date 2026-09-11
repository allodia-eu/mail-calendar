use super::{Offer, Plan, Store, ordered};

fn offer(plan: Plan, store: Store) -> Offer {
    Offer {
        plan,
        store,
        display_price: "\u{20ac}1,99".to_owned(),
    }
}

/// Apple sells a product per plan, so the two plans have to be different strings or the store
/// returns one product for both and every person is billed monthly.
#[test]
fn apple_names_a_product_for_each_plan() {
    let monthly = Plan::Monthly
        .store_product(Store::Apple)
        .expect("apple sells a product");
    let yearly = Plan::Yearly
        .store_product(Store::Apple)
        .expect("apple sells a product");

    assert_ne!(monthly.id, yearly.id);
    assert!(monthly.base_plan.is_none());
    assert!(yearly.base_plan.is_none());
}

/// Play sells one subscription with a base plan per period. Asking it for two different ids is
/// asking for a product that was never published, which it reports as an empty result rather than
/// as an error, so nothing but this test would notice.
#[test]
fn play_names_one_subscription_and_a_base_plan_for_each_plan() {
    let monthly = Plan::Monthly
        .store_product(Store::Google)
        .expect("play sells a subscription");
    let yearly = Plan::Yearly
        .store_product(Store::Google)
        .expect("play sells a subscription");

    assert_eq!(monthly.id, yearly.id);
    assert_eq!(monthly.base_plan.as_deref(), Some("monthly"));
    assert_eq!(yearly.base_plan.as_deref(), Some("yearly"));
}

/// Allodia's own checkout is a web page, not a store product. A caller that reached for one here
/// would be about to ask StoreKit for something Allodia never published.
#[test]
fn allodias_own_checkout_has_no_store_product() {
    for plan in Plan::ALL {
        assert!(plan.store_product(Store::Allodia).is_none());
    }
}

/// The whole reason the ledger exists: a store purchase is money somebody else took, and the
/// account service hears about it only when a client names it.
#[test]
fn only_a_store_purchase_needs_attaching() {
    assert!(!Store::Allodia.needs_attaching());
    assert!(Store::Apple.needs_attaching());
    assert!(Store::Google.needs_attaching());
}

/// Yearly first because it is the cheaper rate, and Allodia's own checkout before a store's
/// because it is the cheaper shop. Both are product decisions, and five clients sorting a list
/// each in their own way is how they stop agreeing.
#[test]
fn the_cheaper_rate_and_the_cheaper_shop_come_first() {
    let drawn = ordered(vec![
        offer(Plan::Monthly, Store::Apple),
        offer(Plan::Yearly, Store::Apple),
        offer(Plan::Monthly, Store::Allodia),
        offer(Plan::Yearly, Store::Allodia),
    ]);

    let order: Vec<_> = drawn
        .iter()
        .map(|offer| (offer.plan, offer.store))
        .collect();
    assert_eq!(
        order,
        vec![
            (Plan::Yearly, Store::Allodia),
            (Plan::Yearly, Store::Apple),
            (Plan::Monthly, Store::Allodia),
            (Plan::Monthly, Store::Apple),
        ]
    );
}

/// A client on a platform with no store, or one whose store returned nothing, draws whatever it
/// does have. Ordering an incomplete list must not drop from it.
#[test]
fn ordering_keeps_every_offer_it_was_given() {
    let drawn = ordered(vec![offer(Plan::Monthly, Store::Google)]);
    assert_eq!(drawn.len(), 1);
    assert!(ordered(Vec::new()).is_empty());
}
