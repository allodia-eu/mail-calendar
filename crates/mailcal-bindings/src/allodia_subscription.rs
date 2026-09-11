// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! What the Allodia account screen draws, and the refusals it has to be able to explain.
//!
//! The rules are the `purchasing.md` contract beside the Allodia Licence. One read answers the
//! whole screen, and `actions` on it decides which buttons exist: a client draws exactly what that
//! says and works nothing out for itself, because the rule it encodes has to know about three
//! billers at once.
//!
//! ⚠️ **None of this gates a capability.** `allodia_entitlement` does. This read is what the
//! screen *says*; gating on it would put a network call in front of a feature.

use crate::allodia_purchase::{AllodiaPlan, AllodiaStore};

/// Who is charging for a plan.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaBiller {
    /// Allodia's own billing: the website and the desktop apps.
    ///
    /// A client says **Allodia**. The service's own word for this is the payment processor behind
    /// it, and naming a processor somebody has never heard of, in a message about being charged
    /// twice, is how a correct warning reads as a scam.
    Allodia,
    /// Apple's App Store.
    Apple,
    /// Google Play.
    Google,
    /// A biller this version cannot name. A client still says that more than one thing is
    /// charging.
    Unknown {
        /// The label the service sent.
        label: String,
    },
}

/// What a store says about a subscription it is billing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaStoreStatus {
    /// Renewing.
    Active,
    /// A charge failed and the store is still retrying. **Access continues**, so this is not a
    /// lapse and must not be drawn as one.
    Grace,
    /// Auto-renewal is off. Access runs to the end of the period already paid for.
    Cancelled,
    /// Play only. Grants nothing.
    OnHold,
    /// Play only. Grants nothing.
    Paused,
    /// The period ran out.
    Expired,
    /// The store took the purchase back, usually a refund. Access stops at once, **whatever the
    /// date beside it says**.
    Revoked,
    /// A status this version does not know. Never read as permission.
    Unknown {
        /// The label the service sent.
        label: String,
    },
}

/// What Allodia's own billing says about the subscription it holds.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaOwnStatus {
    /// A checkout was started and nothing has been charged yet.
    PendingFirstPayment,
    /// Paid up.
    Active,
    /// No further charges. Access runs to the end of the period already paid for.
    Cancelled,
    /// A renewal charge failed and is being retried. Access runs to the same date.
    PastDue,
    /// A status this version does not know.
    Unknown {
        /// The label the service sent.
        label: String,
    },
}

/// The subscription Allodia bills directly. The only one this API can change.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaOwnSubscription {
    /// Where it stands.
    pub status: AllodiaOwnStatus,
    /// Which period it renews on.
    pub interval: AllodiaPlan,
    /// The recurring price in minor units: **what this subscriber is charged**, not today's list
    /// price. A price change never reaches somebody who already subscribed.
    pub amount_in_cents: i64,
    /// ISO 8601, or empty once there is no next charge.
    pub next_payment_date: Option<String>,
    /// ISO 8601. What they have paid through, and when access ends after cancelling.
    pub current_period_end: Option<String>,
    /// ISO 8601, when they cancelled.
    pub cancelled_at: Option<String>,
}

/// A subscription a store is billing. **Read only.**
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaStoreSubscription {
    /// Which store. Never [`AllodiaStore::Allodia`].
    pub source: AllodiaStore,
    /// Where it stands.
    pub status: AllodiaStoreStatus,
    /// Which period it renews on. Display only.
    pub interval: AllodiaPlan,
    /// The store's product id, when it reports one.
    pub product_id: Option<String>,
    /// What the **store** charged, in `currency`. Empty is ordinary for Google Play, which does
    /// not report one; never filled in with Allodia's own price, which the subscriber could
    /// disprove from their receipt.
    pub price_in_cents: Option<i64>,
    /// The currency of `price_in_cents`.
    pub currency: Option<String>,
    /// ISO 8601.
    pub current_period_end: Option<String>,
    /// Whether it renews.
    pub auto_renewing: bool,
    /// `production`, or `sandbox` for a TestFlight or licence-tester purchase.
    pub environment: String,
    /// ⚠️ The store's own account screen, and the **only** thing that can change this
    /// subscription. Cancelling, changing plan and refunding all belong to the store, so a client
    /// opens this rather than offering a button of its own.
    pub manage_url: String,
}

/// What the caller may actually do, as the service decided it.
///
/// **Draw what this says.** Anything else is refused, and the rule behind it has to account for
/// every biller at once, which no client is in a position to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the service's own shape: four independent answers, one per button a screen draws"
)]
pub struct AllodiaSubscriptionActions {
    /// Whether cancelling will work.
    pub can_cancel: bool,
    /// Whether restarting a cancelled subscription will work.
    pub can_resubscribe: bool,
    /// Whether moving between monthly and yearly will work.
    pub can_switch_interval: bool,
    /// Whether starting a checkout will work. **False for anybody already paying through any
    /// source**, which is what stops a desktop build selling a second subscription to somebody the
    /// App Store is already charging.
    pub can_start_checkout: bool,
}

/// Today's list prices for a checkout started through Allodia, in minor units.
///
/// They describe no existing subscription and say nothing about store pricing. A client formats
/// them: the core carries no locale data, so turning 199 and `EUR` into something readable is the
/// platform's job.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaPrices {
    /// The monthly price in minor units.
    pub monthly_in_cents: i64,
    /// The yearly price in minor units.
    pub yearly_in_cents: i64,
    /// ISO 4217.
    pub currency: String,
}

/// Everything the account screen needs, in one read.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaSubscription {
    /// Whether a paid plan is in force right now, from any source. ⚠️ For what the screen
    /// **says**, never for what to switch on.
    pub entitled: bool,
    /// What the caller gets, not what they bought. `free` once every source has lapsed.
    pub plan: String,
    /// ISO 8601. The furthest date any source has been paid through.
    pub current_period_end: Option<String>,
    /// The subscription Allodia bills directly, if there is one.
    pub own: Option<AllodiaOwnSubscription>,
    /// Subscriptions the stores are billing.
    pub stores: Vec<AllodiaStoreSubscription>,
    /// Every source charging for the same plan at once, when there is more than one. Empty is the
    /// normal answer.
    ///
    /// **Reported, never resolved.** A client says which are running and offers each exit;
    /// cancelling one without asking is a decision about somebody's money.
    pub duplicate_billing: Vec<AllodiaBiller>,
    /// What the caller may do.
    pub actions: AllodiaSubscriptionActions,
    /// Today's list prices for a checkout started here.
    pub prices: AllodiaPrices,
    /// False when this deployment has no billing configured at all.
    pub checkout_available: bool,
}

/// Why the service declined, as a stable reason a client writes its own words for.
///
/// **Never the service's own sentence.** A client that renders one ships whatever the service
/// happened to say, which is the rule `allodia_grant_health` already keeps.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AllodiaSubscriptionRefusal {
    /// Cancelling something already cancelled.
    AlreadyCancelled,
    /// Somebody is already being charged, by any source. A second subscription would take money
    /// twice for one plan.
    AlreadyActive,
    /// Already on the period they asked to move to.
    AlreadyOnInterval,
    /// A subscription mid-retry after a failed charge cannot change plan: moving the amount
    /// underneath a retry in flight is how a failed month becomes a charged year.
    NotSwitchable,
    /// There is no such subscription.
    NotFound,
    /// Billing is not configured on this deployment.
    Unavailable,
    /// A reason this version does not know. A client says something general and keeps working.
    Other,
}

/// Where to send somebody to pay, and whether they need to go at all.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaCheckout {
    /// ⚠️ A hosted payment page, opened in a **browser** and never a web view: it carries the
    /// payment-method selection and the recurring-payment authorisation.
    ///
    /// Empty when `reactivated` is true, which only a resubscribe can answer.
    pub checkout_url: Option<String>,
    /// True when a cancelled subscription was restarted on the authorisation already held:
    /// nothing to pay, nothing to re-enter. Always false for a new checkout.
    pub reactivated: bool,
}

/// What a cancellation left behind.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaCancellation {
    /// ISO 8601. What they have paid through, and therefore when access ends.
    ///
    /// ⚠️ **Nothing is refunded**: a month is sold as a month. A client that does not say this
    /// before the confirm button is one whose users think they are getting money back.
    pub end_date: String,
}

/// What switching period actually changed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AllodiaIntervalChange {
    /// The period now in force for the next charge.
    pub interval: AllodiaPlan,
    /// What the next charge will be, in minor units.
    pub amount_in_cents: i64,
    /// ISO 8601, and **unchanged by the switch**. Worth saying before anybody confirms: no money
    /// moves today, and the switch is otherwise silent until a date that may be a month away.
    pub next_payment_date: Option<String>,
}

/// The conversions between these records and the crate that decides with them.
#[cfg(feature = "allodia-license")]
mod from_license {
    use allodia_license::{
        Actions, Biller, Cancelled, Checkout, IntervalChange, OwnStatus, OwnSubscription, Prices,
        Refusal, Resubscribed, StoreStatus, StoreSubscription, Subscription,
    };

    use super::{
        AllodiaBiller, AllodiaCancellation, AllodiaCheckout, AllodiaIntervalChange,
        AllodiaOwnStatus, AllodiaOwnSubscription, AllodiaPrices, AllodiaStoreStatus,
        AllodiaStoreSubscription, AllodiaSubscription, AllodiaSubscriptionActions,
        AllodiaSubscriptionRefusal,
    };
    use crate::allodia_purchase::AllodiaPlan;

    impl From<Biller> for AllodiaBiller {
        fn from(biller: Biller) -> Self {
            match biller {
                Biller::Allodia => Self::Allodia,
                Biller::Apple => Self::Apple,
                Biller::Google => Self::Google,
                Biller::Unknown(label) => Self::Unknown { label },
            }
        }
    }

    impl From<StoreStatus> for AllodiaStoreStatus {
        fn from(status: StoreStatus) -> Self {
            match status {
                StoreStatus::Active => Self::Active,
                StoreStatus::Grace => Self::Grace,
                StoreStatus::Cancelled => Self::Cancelled,
                StoreStatus::OnHold => Self::OnHold,
                StoreStatus::Paused => Self::Paused,
                StoreStatus::Expired => Self::Expired,
                StoreStatus::Revoked => Self::Revoked,
                StoreStatus::Unknown(label) => Self::Unknown { label },
            }
        }
    }

    impl From<OwnStatus> for AllodiaOwnStatus {
        fn from(status: OwnStatus) -> Self {
            match status {
                OwnStatus::PendingFirstPayment => Self::PendingFirstPayment,
                OwnStatus::Active => Self::Active,
                OwnStatus::Cancelled => Self::Cancelled,
                OwnStatus::PastDue => Self::PastDue,
                OwnStatus::Unknown(label) => Self::Unknown { label },
            }
        }
    }

    impl From<OwnSubscription> for AllodiaOwnSubscription {
        fn from(own: OwnSubscription) -> Self {
            Self {
                status: own.status.into(),
                interval: own.interval.into(),
                amount_in_cents: own.amount_in_cents,
                next_payment_date: own.next_payment_date,
                current_period_end: own.current_period_end,
                cancelled_at: own.cancelled_at,
            }
        }
    }

    impl From<StoreSubscription> for AllodiaStoreSubscription {
        fn from(store: StoreSubscription) -> Self {
            Self {
                source: store.source.into(),
                status: store.status.into(),
                interval: store.interval.into(),
                product_id: store.product_id,
                price_in_cents: store.price_in_cents,
                currency: store.currency,
                current_period_end: store.current_period_end,
                auto_renewing: store.auto_renewing,
                environment: store.environment,
                manage_url: store.manage_url,
            }
        }
    }

    impl From<Actions> for AllodiaSubscriptionActions {
        fn from(actions: Actions) -> Self {
            Self {
                can_cancel: actions.can_cancel,
                can_resubscribe: actions.can_resubscribe,
                can_switch_interval: actions.can_switch_interval,
                can_start_checkout: actions.can_start_checkout,
            }
        }
    }

    impl From<Prices> for AllodiaPrices {
        fn from(prices: Prices) -> Self {
            Self {
                monthly_in_cents: prices.monthly_in_cents,
                yearly_in_cents: prices.yearly_in_cents,
                currency: prices.currency,
            }
        }
    }

    impl From<Subscription> for AllodiaSubscription {
        fn from(subscription: Subscription) -> Self {
            Self {
                entitled: subscription.entitled,
                plan: subscription.plan,
                current_period_end: subscription.current_period_end,
                own: subscription.own.map(Into::into),
                stores: subscription.stores.into_iter().map(Into::into).collect(),
                duplicate_billing: subscription
                    .duplicate_billing
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                actions: subscription.actions.into(),
                prices: subscription.prices.into(),
                checkout_available: subscription.checkout_available,
            }
        }
    }

    impl From<Refusal> for AllodiaSubscriptionRefusal {
        fn from(refusal: Refusal) -> Self {
            match refusal {
                Refusal::AlreadyCancelled => Self::AlreadyCancelled,
                Refusal::AlreadyActive => Self::AlreadyActive,
                Refusal::AlreadyOnInterval => Self::AlreadyOnInterval,
                Refusal::NotSwitchable => Self::NotSwitchable,
                Refusal::NotFound => Self::NotFound,
                Refusal::Unavailable => Self::Unavailable,
                Refusal::Other(_) => Self::Other,
            }
        }
    }

    impl From<Checkout> for AllodiaCheckout {
        fn from(checkout: Checkout) -> Self {
            Self {
                checkout_url: Some(checkout.checkout_url),
                reactivated: false,
            }
        }
    }

    impl From<Resubscribed> for AllodiaCheckout {
        fn from(resubscribed: Resubscribed) -> Self {
            Self {
                checkout_url: resubscribed.checkout_url,
                reactivated: resubscribed.reactivated,
            }
        }
    }

    impl From<Cancelled> for AllodiaCancellation {
        fn from(cancelled: Cancelled) -> Self {
            Self {
                end_date: cancelled.end_date,
            }
        }
    }

    impl From<IntervalChange> for AllodiaIntervalChange {
        fn from(changed: IntervalChange) -> Self {
            Self {
                interval: if changed.interval == "yearly" {
                    AllodiaPlan::Yearly
                } else {
                    AllodiaPlan::Monthly
                },
                amount_in_cents: changed.amount_in_cents,
                next_payment_date: changed.next_payment_date,
            }
        }
    }
}
