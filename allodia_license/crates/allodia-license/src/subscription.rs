//! What the account screen shows, and the four things it can do.
//!
//! `GET /api/v1/subscription` answers everything one screen needs in a single read: the
//! subscription Allodia bills directly, any bought through the App Store or Google Play, and what
//! the caller may actually do about each.
//!
//! ⚠️ **This is not what gates a capability.** `GET /entitlement` is, and nothing here may be used
//! for it ([`entitlement.md`](../../../entitlement.md)). The two answer different questions: that
//! one is "what do I switch on", asked constantly and cached for thirty days; this one is "what do
//! I say on this screen", asked when somebody opens it. Gating on this one would put a network
//! read in front of a feature, which is the one rule the entitlement contract has.
//!
//! **The service decides what may be offered, and a client draws exactly that.** [`Actions`] is
//! passed through rather than re-derived: it already accounts for every source, so a desktop build
//! cannot offer to sell a second subscription to somebody the App Store is charging. Working the
//! same thing out locally would be a second copy of a rule that has to know about three billers.

use serde::Deserialize;

use crate::{
    AccountService, Error, Method, Request, Response, Transport,
    purchase::{Plan, Store},
};

/// Where the account screen reads from, relative to the account service's root.
const SUBSCRIPTION_PATH: &str = "/subscription";

/// Who is charging for a plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Biller {
    /// Allodia's own billing: the website and the desktop apps.
    ///
    /// The service calls this `mollie`, after the payment processor behind it. A client says
    /// **Allodia**: that is who the person has a relationship with, and naming a processor they
    /// have never heard of in a message about being charged twice is how a correct warning reads
    /// as a scam.
    Allodia,
    /// Apple's App Store.
    Apple,
    /// Google Play.
    Google,
    /// A biller this version does not know. Kept rather than dropped, so a client can still say
    /// that more than one thing is charging even when it cannot name them all.
    Unknown(String),
}

impl Biller {
    fn parse(label: &str) -> Self {
        match label {
            "mollie" => Self::Allodia,
            "apple" => Self::Apple,
            "google" => Self::Google,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// What a store says about a subscription it is billing.
///
/// The two that are not obvious are the ones that decide whether somebody still has what they paid
/// for: `Grace` is a failed charge the store is still retrying, so access continues, and `Revoked`
/// is the store taking the purchase back, usually a refund, where access stops at once **whatever
/// the date beside it says**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreStatus {
    /// Renewing.
    Active,
    /// A charge failed and the store is still retrying. Access continues.
    Grace,
    /// Auto-renewal is off. Access runs to the end of the period already paid for.
    Cancelled,
    /// Play only. Grants nothing.
    OnHold,
    /// Play only. Grants nothing.
    Paused,
    /// The period ran out.
    Expired,
    /// The store took the purchase back. Access stops at once.
    Revoked,
    /// A status this version does not know. Never drawn as anything in particular, and never read
    /// as permission.
    Unknown(String),
}

impl StoreStatus {
    fn parse(label: &str) -> Self {
        match label {
            "active" => Self::Active,
            "grace" => Self::Grace,
            "cancelled" => Self::Cancelled,
            "on_hold" => Self::OnHold,
            "paused" => Self::Paused,
            "expired" => Self::Expired,
            "revoked" => Self::Revoked,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// What Allodia's own billing says about the subscription it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnStatus {
    /// A checkout was started and nothing has been charged yet.
    PendingFirstPayment,
    /// Paid up.
    Active,
    /// No further charges. Access runs to the end of the period already paid for.
    Cancelled,
    /// A renewal charge failed and is being retried. Access runs to the same date.
    PastDue,
    /// A status this version does not know.
    Unknown(String),
}

impl OwnStatus {
    fn parse(label: &str) -> Self {
        match label {
            "pending_first_payment" => Self::PendingFirstPayment,
            "active" => Self::Active,
            "cancelled" => Self::Cancelled,
            "past_due" => Self::PastDue,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// The subscription Allodia bills directly. The only one this API can change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnSubscription {
    /// Where it stands.
    pub status: OwnStatus,
    /// Which period it renews on.
    pub interval: Plan,
    /// The recurring price in minor units, which is **what this subscriber is charged** rather
    /// than today's list price. A price change never reaches somebody who already subscribed.
    pub amount_in_cents: i64,
    /// ISO 8601, or `None` once there is no next charge.
    pub next_payment_date: Option<String>,
    /// ISO 8601. What they have paid through, and therefore when access ends after cancelling.
    pub current_period_end: Option<String>,
    /// ISO 8601, when they cancelled.
    pub cancelled_at: Option<String>,
}

/// A subscription a store is billing. **Read only**: every change to one happens at its
/// [`manage_url`](Self::manage_url).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSubscription {
    /// Which store. Never [`Store::Allodia`].
    pub source: Store,
    /// Where it stands.
    pub status: StoreStatus,
    /// Which period it renews on. Display only: the store decides what to charge and when.
    pub interval: Plan,
    /// The store's product id, when it reports one.
    pub product_id: Option<String>,
    /// What the **store** charged, in [`currency`](Self::currency), when it reports one. `None` is
    /// ordinary for Google Play, which does not.
    ///
    /// Never Allodia's own euro price: a store sets its own local price, so quoting ours would be
    /// a figure the subscriber can disprove from their receipt.
    pub price_in_cents: Option<i64>,
    /// The currency of [`price_in_cents`](Self::price_in_cents).
    pub currency: Option<String>,
    /// ISO 8601.
    pub current_period_end: Option<String>,
    /// Whether it renews.
    pub auto_renewing: bool,
    /// `production`, or `sandbox` for a TestFlight or licence-tester purchase.
    pub environment: String,
    /// The store's own account screen, and the **only** thing that can change this subscription.
    ///
    /// Cancelling, changing plan and refunding all belong to the store, so a client opens this
    /// rather than calling anything here. Sending somebody to Allodia's page instead is how a
    /// cancellation quietly does not happen.
    pub manage_url: String,
}

/// What the caller may actually do, as the service decided it.
///
/// **Drawn, never re-derived.** Anything not offered here is refused if attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the service's own shape: four independent answers, asked one at a time by whichever \
              button is being drawn. Folding them into a set would be a second vocabulary for a \
              closed list the service already decided."
)]
pub struct Actions {
    /// Whether cancelling will work.
    pub can_cancel: bool,
    /// Whether restarting a cancelled subscription will work.
    pub can_resubscribe: bool,
    /// Whether moving between monthly and yearly will work.
    pub can_switch_interval: bool,
    /// Whether starting a checkout will work.
    ///
    /// **False for anybody already paying through any source, on any platform**, which is what
    /// stops a desktop build selling a second subscription to somebody the App Store is already
    /// charging.
    pub can_start_checkout: bool,
}

/// Today's list prices, in minor units, for a checkout started through this API.
///
/// They describe no existing subscription, and they say nothing about App Store or Play pricing,
/// which each store sets for itself. A client formats them: the core carries no locale data, so
/// turning 199 and `EUR` into a string somebody reads is the platform's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prices {
    /// The monthly price in minor units.
    pub monthly_in_cents: i64,
    /// The yearly price in minor units.
    pub yearly_in_cents: i64,
    /// ISO 4217.
    pub currency: String,
}

impl Prices {
    /// The price of one period, in minor units.
    #[must_use]
    pub const fn of(&self, plan: Plan) -> i64 {
        match plan {
            Plan::Monthly => self.monthly_in_cents,
            Plan::Yearly => self.yearly_in_cents,
        }
    }
}

/// Everything the account screen needs, in one read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// Whether a paid plan is in force right now, from any source.
    ///
    /// ⚠️ For what the screen **says**, never for what to switch on: that is `capabilities` from
    /// `GET /entitlement`, and only that.
    pub entitled: bool,
    /// What the caller gets, not what they bought. Degrades to `free` once every source has
    /// lapsed.
    pub plan: String,
    /// ISO 8601. The furthest date any source has been paid through.
    pub current_period_end: Option<String>,
    /// The subscription Allodia bills directly, if there is one.
    pub own: Option<OwnSubscription>,
    /// Subscriptions the stores are billing.
    pub stores: Vec<StoreSubscription>,
    /// Every source charging for the same plan at the same time, when there is more than one.
    /// Empty is the normal answer.
    ///
    /// **Reported, never resolved.** A store will sell a second subscription to somebody who
    /// already has one here without asking this service, so a client says which two are running
    /// and offers both exits. Cancelling one of them without asking is a decision about their
    /// money.
    pub duplicate_billing: Vec<Biller>,
    /// What the caller may do.
    pub actions: Actions,
    /// Today's list prices for a checkout started here.
    pub prices: Prices,
    /// False when this deployment has no billing configured at all.
    pub checkout_available: bool,
}

// The service is TypeScript and answers in camelCase.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionBody {
    entitled: bool,
    plan: String,
    #[serde(default)]
    current_period_end: Option<String>,
    #[serde(default)]
    subscription: Option<OwnBody>,
    #[serde(default)]
    store_subscriptions: Vec<StoreBody>,
    #[serde(default)]
    duplicate_billing: Vec<String>,
    actions: ActionsBody,
    prices: PricesBody,
    checkout_available: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnBody {
    status: String,
    interval: String,
    amount_in_cents: i64,
    #[serde(default)]
    next_payment_date: Option<String>,
    #[serde(default)]
    current_period_end: Option<String>,
    #[serde(default)]
    cancelled_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreBody {
    source: String,
    status: String,
    interval: String,
    #[serde(default)]
    product_id: Option<String>,
    #[serde(default)]
    price_in_cents: Option<i64>,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    current_period_end: Option<String>,
    auto_renewing: bool,
    environment: String,
    manage_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "mirrors the wire object exactly, which is the point of a body type"
)]
struct ActionsBody {
    can_cancel: bool,
    can_resubscribe: bool,
    can_switch_interval: bool,
    can_start_checkout: bool,
}

#[derive(Deserialize)]
struct PricesBody {
    monthly: i64,
    yearly: i64,
    currency: String,
}

/// Which period a label names.
///
/// The service's enum is closed to these two, so an unrecognised one is a service this build does
/// not understand rather than a third plan: it reads as monthly, which is the cheaper of the two
/// to quote wrongly, and the screen's other fields still say what is actually being charged.
fn interval(label: &str) -> Plan {
    if label == "yearly" {
        Plan::Yearly
    } else {
        Plan::Monthly
    }
}

/// Which store a `source` names. `apple` and `google` are the only two the service sends.
fn source(label: &str) -> Store {
    match label {
        "google" => Store::Google,
        _ => Store::Apple,
    }
}

impl AccountService {
    /// Read everything the account screen needs.
    ///
    /// **Not on a path anyone waits for a capability on.** This is a screen read: it is made when
    /// somebody opens the screen, and what gates a feature is the entitlement cache, which never
    /// blocks.
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing; [`Error::Transport`] when the
    /// request never arrived; [`Error::Malformed`] when the answer cannot be read.
    pub fn subscription(
        &self,
        transport: &dyn Transport,
        access_token: &str,
    ) -> Result<Subscription, Error> {
        let response = transport
            .send(&Request::get(
                format!(
                    "{}{}{SUBSCRIPTION_PATH}",
                    self.base_url(),
                    crate::API_BASE_PATH
                ),
                access_token,
            ))
            .map_err(Error::Transport)?;
        let body = match response.status {
            200..=299 => response.body,
            401 | 403 => return Err(Error::Unauthorized),
            status => return Err(Error::Unexpected { status }),
        };
        read(&body)
    }
}

/// Turn the service's answer into the shape a client draws.
fn read(body: &str) -> Result<Subscription, Error> {
    let parsed: SubscriptionBody =
        serde_json::from_str(body).map_err(|error| Error::Malformed(error.to_string()))?;
    Ok(Subscription {
        entitled: parsed.entitled,
        plan: parsed.plan,
        current_period_end: parsed.current_period_end,
        own: parsed.subscription.map(|own| OwnSubscription {
            status: OwnStatus::parse(&own.status),
            interval: interval(&own.interval),
            amount_in_cents: own.amount_in_cents,
            next_payment_date: own.next_payment_date,
            current_period_end: own.current_period_end,
            cancelled_at: own.cancelled_at,
        }),
        stores: parsed
            .store_subscriptions
            .into_iter()
            .map(|store| StoreSubscription {
                source: source(&store.source),
                status: StoreStatus::parse(&store.status),
                interval: interval(&store.interval),
                product_id: store.product_id,
                price_in_cents: store.price_in_cents,
                currency: store.currency,
                current_period_end: store.current_period_end,
                auto_renewing: store.auto_renewing,
                environment: store.environment,
                manage_url: store.manage_url,
            })
            .collect(),
        duplicate_billing: parsed
            .duplicate_billing
            .iter()
            .map(|label| Biller::parse(label))
            .collect(),
        actions: Actions {
            can_cancel: parsed.actions.can_cancel,
            can_resubscribe: parsed.actions.can_resubscribe,
            can_switch_interval: parsed.actions.can_switch_interval,
            can_start_checkout: parsed.actions.can_start_checkout,
        },
        prices: Prices {
            monthly_in_cents: parsed.prices.monthly,
            yearly_in_cents: parsed.prices.yearly,
            currency: parsed.prices.currency,
        },
        checkout_available: parsed.checkout_available,
    })
}

/// Which method and body one write uses, so the four of them share a request builder.
pub(crate) fn write_request(
    base_url: &str,
    path: &str,
    access_token: &str,
    body: Option<String>,
) -> Request {
    Request {
        url: format!("{base_url}{}{path}", crate::API_BASE_PATH),
        bearer: access_token.to_owned(),
        method: Method::Post,
        // The service takes an object on every one of these, empty where there is nothing to say.
        body: Some(body.unwrap_or_else(|| "{}".to_owned())),
        idempotency_key: None,
    }
}

/// Read a write's answer, or the refusal behind it.
pub(crate) fn answered<T: for<'a> Deserialize<'a>>(response: &Response) -> Result<T, Error> {
    match response.status {
        200..=299 => serde_json::from_str(&response.body)
            .map_err(|error| Error::Malformed(error.to_string())),
        401 | 403 => Err(Error::Unauthorized),
        _ => Err(Error::Refused(crate::Refusal::read(&response.body))),
    }
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
mod tests;
