// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What a client sees of purchasing, asserted where a client sees it.
//!
//! The rules themselves are tested in `allodia-license`, which has no network and no clock. What
//! is tested here is the half that crosses the FFI: that every plan survives the trip, that the
//! product identifiers a store is asked for are the published ones, and that a build carrying no
//! registration sells nothing rather than offering a dead route.

use std::sync::mpsc;

use crate::{
    LogLevel, MailcalApp,
    allodia_purchase::{AllodiaOffer, AllodiaPlan, AllodiaStore},
    tests::{ChannelObserver, NullLogger},
};

fn app() -> std::sync::Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    )
}

fn offer(plan: AllodiaPlan, store: AllodiaStore) -> AllodiaOffer {
    AllodiaOffer {
        plan,
        store,
        display_price: "\u{20ac}1,99".to_owned(),
    }
}

/// Allodia's own checkout is a web page, so there is no store product to ask for. A client that
/// got one here would be about to ask StoreKit for something Allodia never published.
#[test]
fn allodias_own_checkout_offers_no_store_product() {
    assert!(
        app()
            .allodia_store_products(AllodiaStore::Allodia)
            .is_empty()
    );
}

/// A build from source carries no registration, sells nothing, and says so rather than putting a
/// purchase button in front of somebody it cannot charge.
#[cfg(not(feature = "allodia-license"))]
mod without_a_registration {
    use super::{AllodiaStore, app};
    use crate::allodia_purchase::AllodiaPurchaseError;

    #[test]
    fn no_store_is_asked_for_anything() {
        for store in [
            AllodiaStore::Allodia,
            AllodiaStore::Apple,
            AllodiaStore::Google,
        ] {
            assert!(app().allodia_store_products(store).is_empty());
        }
    }

    /// Ordering is a pass-through here rather than a sort, because the sort lives in the crate
    /// this build does not link. Nothing draws offers in a build that sells nothing, so what
    /// matters is only that a caller is handed back what it gave rather than losing it.
    #[test]
    fn ordering_offers_keeps_every_one_it_was_given() {
        use super::{AllodiaPlan, offer};

        let given = vec![
            offer(AllodiaPlan::Monthly, AllodiaStore::Apple),
            offer(AllodiaPlan::Yearly, AllodiaStore::Allodia),
        ];

        assert_eq!(app().order_allodia_offers(given.clone()), given);
    }

    #[test]
    fn there_is_nothing_to_attach_a_purchase_to() {
        let refused = app()
            .link_allodia_purchases(Vec::new())
            .expect_err("a build that sells nothing cannot attach a purchase");

        assert!(matches!(refused, AllodiaPurchaseError::Unavailable));
    }
}

#[cfg(feature = "allodia-license")]
mod with_a_registration {
    use allodia_license::Plan;

    use super::{AllodiaPlan, AllodiaStore, app};
    use crate::allodia_purchase::{AllodiaPurchaseError, AllodiaStorePurchase};

    /// Yearly before monthly because it is the cheaper rate, and Allodia's own checkout before a
    /// store's because it is the cheaper shop. Decided in the core so five clients cannot each
    /// sort the list their own way, and asserted here because this is where a client reads it.
    #[test]
    fn the_cheaper_rate_and_the_cheaper_shop_are_drawn_first() {
        use super::offer;

        let drawn = app().order_allodia_offers(vec![
            offer(AllodiaPlan::Monthly, AllodiaStore::Apple),
            offer(AllodiaPlan::Yearly, AllodiaStore::Apple),
            offer(AllodiaPlan::Monthly, AllodiaStore::Allodia),
            offer(AllodiaPlan::Yearly, AllodiaStore::Allodia),
        ]);

        let order: Vec<_> = drawn
            .iter()
            .map(|offer| (offer.plan, offer.store))
            .collect();
        assert_eq!(
            order,
            vec![
                (AllodiaPlan::Yearly, AllodiaStore::Allodia),
                (AllodiaPlan::Yearly, AllodiaStore::Apple),
                (AllodiaPlan::Monthly, AllodiaStore::Allodia),
                (AllodiaPlan::Monthly, AllodiaStore::Apple),
            ]
        );
    }

    /// The FFI enum and the crate's own are two lists that have to stay the same length. A variant
    /// added to one and not the other is a plan a client can never name.
    #[test]
    fn every_plan_the_core_sells_survives_the_trip() {
        for plan in Plan::ALL {
            assert_eq!(Plan::from(AllodiaPlan::from(*plan)), *plan);
        }
    }

    /// Apple sells a product per period; two plans sharing an id would bill everybody monthly.
    #[test]
    fn apple_is_asked_for_a_product_per_period() {
        let products = app().allodia_store_products(AllodiaStore::Apple);

        assert_eq!(products.len(), 2);
        assert_ne!(products[0].product_id, products[1].product_id);
        assert!(products.iter().all(|product| product.base_plan.is_none()));
    }

    /// Play sells one subscription with a base plan per period. Asking it for two ids is asking
    /// for a product that was never published, which Play reports as an empty result rather than
    /// as an error, so nothing but this would notice.
    #[test]
    fn play_is_asked_for_one_subscription_and_a_base_plan_per_period() {
        let products = app().allodia_store_products(AllodiaStore::Google);

        assert_eq!(products.len(), 2);
        assert_eq!(products[0].product_id, products[1].product_id);
        assert!(products.iter().all(|product| product.base_plan.is_some()));
    }

    /// A purchase is attached to an account, so with nobody signed in there is nothing to attach
    /// it to. **Nothing is settled**, which is what leaves the purchase recoverable: the store
    /// keeps offering it until somebody signs in and the pass can run.
    #[test]
    fn a_purchase_made_with_nobody_signed_in_is_not_finished() {
        let refused = app()
            .link_allodia_purchases(vec![AllodiaStorePurchase {
                store: AllodiaStore::Apple,
                purchase: "t-1".to_owned(),
                purchased_at: 1_760_000_000,
            }])
            .expect_err("nobody is signed in");

        assert!(matches!(refused, AllodiaPurchaseError::NotSignedIn));
    }
}
