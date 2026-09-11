// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The purchasing FFI methods on [`MailcalApp`]. The records they carry are in
//! [`crate::allodia_purchase`], and the rules they apply are `purchasing.md`'s, the contract
//! beside the Allodia Licence.
//!
//! Three calls, and a client needs no other knowledge of any of this. Two are local and instant,
//! so a client may use them while drawing a screen. The third makes network round trips and
//! **blocks**, exactly as `sync_allodia_accounts` does, so a host calls it off the main thread.
//!
//! **What stays on the client's side.** Asking StoreKit or the Play Billing Library for products,
//! putting up the purchase sheet, listening for transactions, and finishing them once this pass
//! says to. None of that can be done from Rust, and none of it decides anything.

use crate::{
    MailcalApp,
    allodia_purchase::{
        AllodiaOffer, AllodiaPurchaseError, AllodiaPurchaseReport, AllodiaStore,
        AllodiaStoreProduct, AllodiaStorePurchase,
    },
};

#[uniffi::export]
impl MailcalApp {
    /// What to ask this platform's store for, one entry per period.
    ///
    /// Local and instant. The identifiers are constants, because Apple never releases a product id
    /// for reuse and Play refuses to change one after publication, and they are here rather than
    /// in five clients so that a console and an app cannot disagree about what was published.
    ///
    /// Empty for [`AllodiaStore::Allodia`], which sells through a web checkout and has no store
    /// product.
    #[must_use]
    #[cfg_attr(
        not(feature = "allodia-license"),
        expect(
            unused_variables,
            reason = "a build with no registration sells nothing, so it asks no store which"
        )
    )]
    pub fn allodia_store_products(&self, store: AllodiaStore) -> Vec<AllodiaStoreProduct> {
        #[cfg(not(feature = "allodia-license"))]
        {
            Vec::new()
        }
        #[cfg(feature = "allodia-license")]
        {
            allodia_license::Plan::ALL
                .iter()
                .filter_map(|plan| {
                    plan.store_product(store.into())
                        .map(|product| AllodiaStoreProduct {
                            plan: (*plan).into(),
                            product_id: product.id.as_str().to_owned(),
                            base_plan: product.base_plan,
                        })
                })
                .collect()
        }
    }

    /// The offers a client has gathered, in the order to draw them.
    ///
    /// Local and instant, and a pure sort: yearly before monthly because it is the cheaper rate,
    /// and Allodia's own checkout before a store's because it is the cheaper shop. Decided here so
    /// that five clients cannot each sort the list their own way.
    ///
    /// A client passes whatever it actually has. A platform with no store, or a store that
    /// answered with nothing, draws the rest rather than nothing.
    #[must_use]
    pub fn order_allodia_offers(&self, offers: Vec<AllodiaOffer>) -> Vec<AllodiaOffer> {
        #[cfg(not(feature = "allodia-license"))]
        {
            offers
        }
        #[cfg(feature = "allodia-license")]
        {
            allodia_license::ordered(offers.iter().map(Into::into).collect())
                .into_iter()
                .map(Into::into)
                .collect()
        }
    }

    /// Hands the account service every store purchase that has not been granted yet, and says what
    /// to do with the store afterwards.
    ///
    /// `purchases` is **everything the store currently offers as unfinished**, not just a new one:
    /// StoreKit re-delivers at every launch and Play returns the set from every
    /// `queryPurchasesAsync`, and the store is the authority on what is outstanding. A purchase it
    /// has stopped offering is dropped here as a result.
    ///
    /// ⚠️ **Finish only what [`AllodiaPurchaseReport::finish`] names, and only after this
    /// returns.** A purchase finished before the service has granted it is money taken for
    /// nothing: StoreKit would stop re-delivering the only copy of it, and Play would have no
    /// reason to refund it.
    ///
    /// **Blocking**: it mints a token and makes a request per purchase. Call it off the main
    /// thread.
    ///
    /// # Errors
    ///
    /// [`AllodiaPurchaseError::Unavailable`] in a build with no Allodia registration;
    /// [`AllodiaPurchaseError::NotSignedIn`] when there is no account to attach a purchase to;
    /// [`AllodiaPurchaseError::NeedsReauth`] when the stored sign-in predates the permission this
    /// needs; [`AllodiaPurchaseError::Unreachable`] when no token could be minted. Nothing is
    /// finished in any of those cases, which is what leaves the purchase recoverable.
    pub fn link_allodia_purchases(
        &self,
        purchases: Vec<AllodiaStorePurchase>,
    ) -> Result<AllodiaPurchaseReport, AllodiaPurchaseError> {
        #[cfg(not(feature = "allodia-license"))]
        {
            drop(purchases);
            Err(AllodiaPurchaseError::Unavailable)
        }
        #[cfg(feature = "allodia-license")]
        {
            self.run_allodia_link_pass(&purchases)
        }
    }
}

#[cfg(feature = "allodia-license")]
mod pass {
    use allodia_license::{AccountService, Error, Feature, LinkOutcome, Settled, StorePurchase};

    use super::{AllodiaPurchaseError, AllodiaPurchaseReport, AllodiaStorePurchase};
    use crate::{AllodiaGrantHealth, MailcalApp, allodia_transport::HttpsTransport};

    impl MailcalApp {
        /// One redemption pass: bring the ledger into step with the store, attempt what is due,
        /// and report what the client should finish.
        pub(super) fn run_allodia_link_pass(
            &self,
            purchases: &[AllodiaStorePurchase],
        ) -> Result<AllodiaPurchaseReport, AllodiaPurchaseError> {
            let token = self.allodia_access_token().map_err(|_| {
                if self.allodia.lock().expect("allodia account lock").is_none() {
                    AllodiaPurchaseError::NotSignedIn
                } else {
                    AllodiaPurchaseError::Unreachable
                }
            })?;
            // Asked after the token, because minting one is what learns the grant's scopes on an
            // install that never recorded them, and before the first request, because that request
            // cannot succeed and its refusal would read as the service being down.
            if !self.allodia_grant_permits(Feature::WriteSubscription) {
                self.note_allodia_health(AllodiaGrantHealth::NeedsReauth);
                return Err(AllodiaPurchaseError::NeedsReauth);
            }
            let transport = HttpsTransport::new(self.runtime.handle().clone())
                .map_err(|_| AllodiaPurchaseError::Unreachable)?;
            let service = AccountService::new(allodia_license::host());
            let now = time::OffsetDateTime::now_utc().unix_timestamp();

            let owned: Vec<StorePurchase> = purchases.iter().map(Into::into).collect();
            let attempts = {
                let mut ledger = self.allodia_purchases.lock().expect("allodia ledger lock");
                ledger.sync(&owned);
                ledger
                    .due(now)
                    .into_iter()
                    .map(|entry| entry.purchase.clone())
                    .collect::<Vec<_>>()
            };
            if !attempts.is_empty() {
                log::info!("allodia: attaching {} store purchase(s)", attempts.len());
            }

            let mut report = AllodiaPurchaseReport::default();
            for id in attempts {
                let Some(purchase) = owned.iter().find(|owned| owned.purchase == id) else {
                    continue;
                };
                let attempt = self.attempt_link(&service, &transport, &token, purchase);
                let outcome = match attempt {
                    Attempt::Answered(outcome) => outcome,
                    Attempt::TokenRefused => LinkOutcome::Deferred,
                };
                let settled = self
                    .allodia_purchases
                    .lock()
                    .expect("allodia ledger lock")
                    .apply(&id, outcome, now);
                if settled == Settled::Settled {
                    if outcome == LinkOutcome::ClaimedElsewhere {
                        report.claimed_elsewhere.push(id.clone());
                    }
                    report.finish.push(id);
                }
                // A refused token refuses every later call identically, so the pass stops rather
                // than spending a round trip per outstanding purchase to be told the same thing.
                // Everything left keeps its place in the ledger and comes round again.
                if attempt == Attempt::TokenRefused {
                    break;
                }
            }

            let ledger = self.allodia_purchases.lock().expect("allodia ledger lock");
            report.waiting = ledger
                .pending()
                .iter()
                .map(|entry| entry.purchase.clone())
                .collect();
            report.anything_stuck = ledger.has_stuck(now);
            Ok(report)
        }

        /// One purchase, with the refusal shapes folded into what the ledger understands.
        ///
        /// A refused token is [`LinkOutcome::Deferred`] like any other failure to learn something,
        /// and it is recorded against the grant so the rest of the app can say why. What it is
        /// **not** is a reason to settle the purchase: an expired token is this device's problem,
        /// not evidence about somebody's payment.
        fn attempt_link(
            &self,
            service: &AccountService,
            transport: &HttpsTransport,
            token: &str,
            purchase: &StorePurchase,
        ) -> Attempt {
            match service.link_store_purchase(transport, token, purchase) {
                Ok(outcome) => {
                    self.note_allodia_health(AllodiaGrantHealth::Ok);
                    Attempt::Answered(outcome)
                }
                Err(Error::Unauthorized) => {
                    self.note_allodia_health(AllodiaGrantHealth::SignedOut);
                    Attempt::TokenRefused
                }
                Err(_) => Attempt::Answered(LinkOutcome::Deferred),
            }
        }
    }

    /// What one attempt tells the pass, which is more than it tells the ledger.
    ///
    /// The ledger only needs to know that nothing was learned. The pass also needs to know
    /// **whether asking again could go differently**, and a refused token is the one refusal where
    /// it cannot: every remaining purchase would buy the same answer at the price of a round trip.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Attempt {
        Answered(LinkOutcome),
        TokenRefused,
    }
}

#[cfg(test)]
#[path = "app_allodia_purchase_tests.rs"]
mod tests;
