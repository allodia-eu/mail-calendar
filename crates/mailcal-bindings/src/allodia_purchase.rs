// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What a client and the core say to each other about buying a subscription.
//!
//! The rules are the `purchasing.md` contract beside the Allodia Licence; this is the seam.
//! **Everything store-shaped stays on the client's side of it**, because no Rust can call StoreKit
//! or the Play Billing Library: a client asks the store for products, runs the purchase sheet,
//! listens for transactions and finishes them. What crosses is a product identifier going out and
//! a proof coming back.
//!
//! The records are mirrors of `allodia_license`'s own types rather than the types themselves,
//! which is the same arrangement every other Allodia record here has: UniFFI needs its own derives,
//! and a client should not have to learn a crate's vocabulary to draw a price.

/// How long a billing period runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaPlan {
    /// Billed every month.
    Monthly,
    /// Billed every year, at a lower effective rate.
    Yearly,
}

/// Who takes the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaStore {
    /// Allodia's own checkout, opened in a browser. No commission, so the lowest price.
    Allodia,
    /// Apple's App Store, through StoreKit.
    Apple,
    /// Google Play, through the Play Billing Library.
    Google,
}

/// What to ask a store for.
///
/// Apple sells a product per period inside one subscription group, so `base_plan` is empty there.
/// Play sells one subscription with a base plan per period, so the id alone does not name a price
/// and `base_plan` says which one to launch.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaStoreProduct {
    /// Which period this buys.
    pub plan: AllodiaPlan,
    /// Apple: the product's own id. Play: the subscription's id.
    pub product_id: String,
    /// Play: which base plan of that subscription. Empty on Apple, which needs no such thing.
    pub base_plan: Option<String>,
}

/// One thing a person can buy, as the shop selling it described it just now.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaOffer {
    /// Which period this buys.
    pub plan: AllodiaPlan,
    /// Which shop is selling it.
    pub store: AllodiaStore,
    /// The price exactly as the shop formatted it, currency symbol and all.
    ///
    /// **Drawn, never parsed and never recomputed.** It is the store's own localised string, the
    /// store's commission is already in it, and the core has no locale data to format a price with
    /// even if it wanted to.
    pub display_price: String,
}

/// A purchase a store says this person owns.
///
/// **An identifier and nothing else**, which is all the service takes: it reads every fact about
/// the subscription back from the store itself, so nothing a device claims about what it bought is
/// believed. There is no period here for the same reason: a Play purchase names its subscription
/// but not which base plan was bought, and only the Play Developer API resolves that.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaStorePurchase {
    /// Which shop sold it, so the service knows whose API to read it back from.
    pub store: AllodiaStore,
    /// Apple's StoreKit transaction identifier, or Play's purchase token.
    pub purchase: String,
    /// When the store says the purchase was made, in Unix seconds.
    ///
    /// StoreKit's `purchaseDate`, Play's `getPurchaseTime`. Taken from the store rather than from
    /// the device's own clock, so it survives a relaunch and cannot be skewed.
    pub purchased_at: i64,
}

/// What one redemption pass did, and what the client has left to do.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaPurchaseReport {
    /// Purchases the service is done with, which an **Apple** client finishes with StoreKit.
    ///
    /// Nothing to do on Play: the service acknowledges a Play purchase itself when it attaches it,
    /// which is what Play requires within three days or it refunds the purchase. A StoreKit
    /// transaction can only be finished by the device holding it, so there it stays a client's
    /// job, and until the client does it StoreKit keeps offering the transaction, which is the
    /// safety net rather than a bug.
    pub finish: Vec<String>,
    /// Of those, the ones settled because a **different** Allodia account already owns them, which
    /// is what restoring purchases onto a second Allodia account looks like. Nothing was granted
    /// to this one, and a client says so.
    pub claimed_elsewhere: Vec<String>,
    /// Still waiting, and to be left alone. Either the pass could not reach the service, or the
    /// purchase is inside its retry wait.
    pub waiting: Vec<String>,
    /// Whether some purchase has been waiting long enough to be worth telling the person about.
    ///
    /// Money was taken and nothing has been granted. A client that draws nothing here leaves them
    /// with no way to find out.
    pub anything_stuck: bool,
}

/// The conversions between these records and the crate that decides with them.
///
/// Behind the feature for the same reason the records in front of it are not: a client draws the
/// same screen whether or not this build carries an Allodia registration, and a build without one
/// has no `allodia_license` to convert to.
#[cfg(feature = "allodia-license")]
mod from_license {
    use allodia_license::{Offer, Plan, Store, StorePurchase};

    use super::{AllodiaOffer, AllodiaPlan, AllodiaStore, AllodiaStorePurchase};

    impl From<AllodiaPlan> for Plan {
        fn from(plan: AllodiaPlan) -> Self {
            match plan {
                AllodiaPlan::Monthly => Self::Monthly,
                AllodiaPlan::Yearly => Self::Yearly,
            }
        }
    }

    impl From<Plan> for AllodiaPlan {
        fn from(plan: Plan) -> Self {
            match plan {
                Plan::Monthly => Self::Monthly,
                Plan::Yearly => Self::Yearly,
            }
        }
    }

    impl From<AllodiaStore> for Store {
        fn from(store: AllodiaStore) -> Self {
            match store {
                AllodiaStore::Allodia => Self::Allodia,
                AllodiaStore::Apple => Self::Apple,
                AllodiaStore::Google => Self::Google,
            }
        }
    }

    impl From<Store> for AllodiaStore {
        fn from(store: Store) -> Self {
            match store {
                Store::Allodia => Self::Allodia,
                Store::Apple => Self::Apple,
                Store::Google => Self::Google,
            }
        }
    }

    impl From<&AllodiaStorePurchase> for StorePurchase {
        fn from(purchase: &AllodiaStorePurchase) -> Self {
            Self {
                store: purchase.store.into(),
                purchase: purchase.purchase.clone(),
                purchased_at: purchase.purchased_at,
            }
        }
    }

    impl From<&AllodiaOffer> for Offer {
        fn from(offer: &AllodiaOffer) -> Self {
            Self {
                plan: offer.plan.into(),
                store: offer.store.into(),
                display_price: offer.display_price.clone(),
            }
        }
    }

    impl From<Offer> for AllodiaOffer {
        fn from(offer: Offer) -> Self {
            Self {
                plan: offer.plan.into(),
                store: offer.store.into(),
                display_price: offer.display_price,
            }
        }
    }
}

/// Why a purchase surface could not do what was asked.
///
/// Separate from [`MailcalError`](crate::MailcalError) because none of these is a mail failure and
/// a client draws them on a card of its own, and because the first of them is the ordinary answer
/// for a build from source rather than something going wrong.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AllodiaPurchaseError {
    /// This build carries no Allodia registration, so it sells nothing. A client checks
    /// `allodia_sign_in_available()` and draws no purchase surface at all.
    #[error("this build has no Allodia account surface")]
    Unavailable,
    /// Nobody is signed in. A purchase is attached to an account, so there is nothing to attach
    /// it to; the client offers the sign-in first.
    #[error("no Allodia account is signed in")]
    NotSignedIn,
    /// The stored sign-in predates the permission this needs. A client offers signing in again.
    #[error(
        "this device's Allodia sign-in does not yet include permission to change a subscription"
    )]
    NeedsReauth,
    /// The account service could not be reached. Nothing was learned and nothing was finished.
    #[error("the account service could not be reached")]
    Unreachable,
    /// The service declined, and said why as a stable reason.
    ///
    /// A client switches on `reason` and writes its own words. **Never** the service's own
    /// sentence, which is the rule `allodia_grant_health` already keeps: a client that renders one
    /// ships whatever the service happened to say.
    #[error("the account service declined")]
    Refused {
        /// Why.
        reason: crate::allodia_subscription::AllodiaSubscriptionRefusal,
    },
}
