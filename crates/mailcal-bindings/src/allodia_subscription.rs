// SPDX-FileCopyrightText: 2026 Allodia
// SPDX-License-Identifier: GPL-3.0-only

//! The service's answer about an account's subscription, which the core decides the subscription
//! section from (`allodia_subscription_facts`), and the refusals a write can meet.
//!
//! The rules are the `purchasing.md` contract beside the Allodia Licence. `actions` decides which
//! writes exist, because the rule it encodes has to know about three billers at once.
//!
//! ⚠️ **None of this gates a capability.** `allodia_entitlement` does. This read is what the
//! section *says*; gating on it would put a network call in front of a feature.

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
#[cfg_attr(
    not(any(feature = "allodia-license", test)),
    expect(dead_code, reason = "only the service's answer builds one")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AllodiaStoreStatus {
    /// Renewing.
    Active,
    /// A charge failed and the store is still retrying. **Access continues**, so this is not a
    /// lapse and must not be drawn as one.
    Grace,
    /// Auto-renewal is off. Access runs to the end of the period already paid for.
    Cancelled,
    /// Play only. Grants nothing, and is fixed only at the store.
    OnHold,
    /// Play only. Grants nothing, and is lifted only at the store.
    Paused,
    /// The period ran out.
    Expired,
    /// The store took the purchase back, usually a refund. Access stops at once, **whatever the
    /// date beside it says**.
    Revoked,
    /// A status this version does not know. Never read as permission.
    Unknown,
}

/// What Allodia's own billing says about the subscription it holds.
#[cfg_attr(
    not(any(feature = "allodia-license", test)),
    expect(dead_code, reason = "only the service's answer builds one")
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AllodiaOwnStatus {
    /// A checkout was started and nothing has been charged yet.
    PendingFirstPayment,
    /// Paid up.
    Active,
    /// No further charges. Access runs to the end of the period already paid for.
    Cancelled,
    /// A renewal charge failed and is being retried. Access runs to the same date.
    PastDue,
    /// A status this version does not know.
    Unknown,
}

/// The subscription Allodia bills directly. The only one this API can change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaOwnSubscription {
    pub(crate) status: AllodiaOwnStatus,
    /// Which period it renews on.
    pub(crate) interval: AllodiaPlan,
    /// ISO 8601, or empty once there is no next charge.
    pub(crate) next_payment_date: Option<String>,
    /// ISO 8601. What they have paid through, and when access ends after cancelling.
    pub(crate) current_period_end: Option<String>,
}

/// A subscription a store is billing. **Read only**: changed at that store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaStoreSubscription {
    /// Which store. Never [`AllodiaStore::Allodia`].
    pub(crate) source: AllodiaStore,
    pub(crate) status: AllodiaStoreStatus,
    pub(crate) auto_renewing: bool,
    /// The store's own account screen, the only thing that can change this subscription.
    pub(crate) manage_url: String,
}

/// What the caller may do, as the service decided it. Anything else is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the service's own shape: four independent answers, one per write"
)]
pub(crate) struct AllodiaSubscriptionActions {
    pub(crate) can_cancel: bool,
    pub(crate) can_resubscribe: bool,
    pub(crate) can_switch_interval: bool,
    /// **False for anybody already paying through any source**, which is what stops a desktop
    /// build selling a second subscription to somebody the App Store is already charging.
    pub(crate) can_start_checkout: bool,
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

/// The service's whole answer about an account's subscription, from which
/// [`crate::AllodiaSubscriptionView`] is decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaSubscription {
    /// Whether a paid plan is in force right now, from any source. For what the section **says**,
    /// never for what to switch on.
    pub(crate) entitled: bool,
    /// ISO 8601. The furthest date any source has been paid through.
    pub(crate) current_period_end: Option<String>,
    pub(crate) own: Option<AllodiaOwnSubscription>,
    pub(crate) stores: Vec<AllodiaStoreSubscription>,
    /// Every source charging for the same plan at once, when there is more than one.
    pub(crate) duplicate_billing: Vec<AllodiaBiller>,
    pub(crate) actions: AllodiaSubscriptionActions,
    pub(crate) prices: AllodiaPrices,
    /// False when this deployment has no billing configured at all.
    pub(crate) checkout_available: bool,
}

/// Why the service declined, as a stable reason a client writes its own words for.
///
/// **Never the service's own sentence.** A client that renders one ships whatever the service
/// happened to say.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaCheckout {
    /// ⚠️ A hosted payment page, opened in a **browser** and never a web view: it carries the
    /// payment-method selection and the recurring-payment authorisation. Empty when
    /// `reactivated`.
    pub(crate) checkout_url: Option<String>,
    /// A cancelled subscription restarted on the authorisation already held: nothing to pay,
    /// nothing to re-enter. Always false for a new checkout.
    pub(crate) reactivated: bool,
}

/// What switching period changed: the next charge, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllodiaIntervalChange {
    /// What the next charge will be, in minor units.
    pub(crate) amount_in_cents: i64,
    /// ISO 8601, and **unchanged by the switch**.
    pub(crate) next_payment_date: Option<String>,
}

/// The conversions between these records and the crate that decides with them.
#[cfg(feature = "allodia-license")]
mod from_license {
    use allodia_license::{
        Actions, Biller, Checkout, IntervalChange, OwnStatus, OwnSubscription, Prices, Refusal,
        Resubscribed, StoreStatus, StoreSubscription, Subscription,
    };

    use super::{
        AllodiaBiller, AllodiaCheckout, AllodiaIntervalChange, AllodiaOwnStatus,
        AllodiaOwnSubscription, AllodiaPrices, AllodiaStoreStatus, AllodiaStoreSubscription,
        AllodiaSubscription, AllodiaSubscriptionActions, AllodiaSubscriptionRefusal,
    };

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
                StoreStatus::Unknown(_) => Self::Unknown,
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
                OwnStatus::Unknown(_) => Self::Unknown,
            }
        }
    }

    impl From<OwnSubscription> for AllodiaOwnSubscription {
        fn from(own: OwnSubscription) -> Self {
            Self {
                status: own.status.into(),
                interval: own.interval.into(),
                next_payment_date: own.next_payment_date,
                current_period_end: own.current_period_end,
            }
        }
    }

    impl From<StoreSubscription> for AllodiaStoreSubscription {
        fn from(store: StoreSubscription) -> Self {
            Self {
                source: store.source.into(),
                status: store.status.into(),
                auto_renewing: store.auto_renewing,
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

    impl From<IntervalChange> for AllodiaIntervalChange {
        fn from(changed: IntervalChange) -> Self {
            Self {
                amount_in_cents: changed.amount_in_cents,
                next_payment_date: changed.next_payment_date,
            }
        }
    }
}
