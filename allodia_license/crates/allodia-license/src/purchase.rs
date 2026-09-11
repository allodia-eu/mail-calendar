//! What a person can buy, and from whom.
//!
//! Three shops sell the same service, and the difference between them is who takes the payment,
//! not what is granted. Allodia's own checkout is one; Apple's and Google's stores are the other
//! two, and each adds its commission to the price, so the same plan costs more inside a phone app
//! than on the website.
//!
//! **No price is written here, and none is computed here.** A store returns its own localised,
//! marked-up price string and that is what a client draws: Apple requires the store's formatted
//! price be shown rather than one the app assembled, currency formatting is the platform's job
//! (the core is tzdata-free and locale-free, [`../../../AGENTS.md`](../../../AGENTS.md)), and a
//! price kept in two places drifts the first time one of them is repriced in a console.
//!
//! **No store payload is parsed here either.** Apple's signed transaction and Google's purchase
//! token are the stores' own formats, verified against the stores' server APIs with keys that
//! exist only on Allodia's servers. A client carries the proof from the store to the service
//! without opening it, so there is no second implementation here to disagree with the one that
//! decides.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How long a billing period runs.
///
/// Two, deliberately. A third would be a third price to keep aligned across three shops and two
/// consoles, and neither store makes that cheap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Plan {
    /// Billed every month.
    Monthly,
    /// Billed every year, at a lower effective rate.
    Yearly,
}

impl Plan {
    /// Both, in the order a client draws them.
    ///
    /// Yearly first, because it is the cheaper of the two per month and a person choosing between
    /// them should see that one first. The order is decided here rather than in each client so
    /// that five of them cannot each decide it differently.
    pub const ALL: &'static [Self] = &[Self::Yearly, Self::Monthly];

    /// A stable label for the wire, and for a log line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    /// What this plan is called in a store's console.
    ///
    /// `None` for [`Store::Allodia`], which sells through a web checkout and has no store product.
    #[must_use]
    pub fn store_product(self, store: Store) -> Option<StoreProduct> {
        match store {
            Store::Allodia => None,
            Store::Apple => Some(StoreProduct {
                id: ProductId::new(format!("{SUBSCRIPTION_ID}.{}", self.as_str())),
                base_plan: None,
            }),
            Store::Google => Some(StoreProduct {
                id: ProductId::new(SUBSCRIPTION_ID),
                base_plan: Some(self.as_str().to_owned()),
            }),
        }
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who takes the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Store {
    /// Allodia's own checkout, opened in a browser. No commission, so the lowest price.
    Allodia,
    /// Apple's App Store, through StoreKit.
    Apple,
    /// Google Play, through the Play Billing Library.
    Google,
}

impl Store {
    /// A stable label for the wire, and for a log line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allodia => "allodia",
            Self::Apple => "apple",
            Self::Google => "google",
        }
    }

    /// Whether a purchase made here has to be carried to the account service by a client.
    ///
    /// Allodia's own checkout is billed against the account already, because the person is signed
    /// in to the one they are buying for. A store purchase is money taken by somebody else, and
    /// the account service learns of it only when a client names it, which is what
    /// [`Ledger`](crate::Ledger) exists to make certain of.
    #[must_use]
    pub const fn needs_attaching(self) -> bool {
        match self {
            Self::Allodia => false,
            Self::Apple | Self::Google => true,
        }
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A store's own identifier for something it sells.
///
/// A newtype because the two stores shape these differently, and mixing them up asks the wrong
/// shop for the wrong thing, which both report as a product that does not exist.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProductId(String);

impl ProductId {
    /// Wrap what a console was told.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The string to hand the store SDK.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The subscription id both stores hold the service under.
///
/// One string, chosen once. Apple never releases a product id for reuse and Play refuses to change
/// one after publication, so this constant is effectively permanent, and that is why it is stated
/// here rather than assembled from the injected application id: an unbranded build has no store
/// presence at all, and a branded one has to reach the products Allodia actually published.
const SUBSCRIPTION_ID: &str = "eu.allodia.mailcal.services";

/// What to ask a store for, which is not the same question on each.
///
/// Apple sells a product per plan inside one subscription group. Play sells **one** subscription
/// carrying a base plan per period, so the id alone does not name a price and the base plan has to
/// travel with it. Modelling both as one shape leaves the Play client asking for a product that
/// does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreProduct {
    /// Apple: the product's own id. Play: the subscription's id.
    pub id: ProductId,
    /// Play: which base plan of that subscription. `None` on Apple, which needs no such thing.
    pub base_plan: Option<String>,
}

/// One thing a person can buy, as the shop selling it described it just now.
///
/// The client fetches these from the store it is running on and hands them over; the core decides
/// what to draw and in which order. Nothing here is stored: a price is only as good as the moment
/// it was read, and a stale one drawn beside a live purchase button is how somebody is charged an
/// amount they were never shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Which period this buys.
    pub plan: Plan,
    /// Which shop is selling it.
    pub store: Store,
    /// The price exactly as the shop formatted it, currency symbol and all.
    ///
    /// Never assembled, never converted, never compared. A client draws this string.
    pub display_price: String,
}

/// Put what the shops returned into the order a client draws them.
///
/// Two rules, both decided once here. Yearly before monthly, because it is the cheaper rate and
/// the one worth seeing first. Allodia's own checkout before a store's, because it is the cheaper
/// shop and a person who is never shown it cannot choose it.
#[must_use]
pub fn ordered(mut offers: Vec<Offer>) -> Vec<Offer> {
    offers.sort_by_key(|offer| {
        (
            Plan::ALL
                .iter()
                .position(|plan| *plan == offer.plan)
                .unwrap_or(usize::MAX),
            offer.store,
        )
    });
    offers
}

#[cfg(test)]
#[path = "purchase_tests.rs"]
mod tests;
