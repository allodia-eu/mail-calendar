//! The four things the account screen can do to Allodia's **own** subscription.
//!
//! A store's subscription is not one of them. Cancelling, changing plan and refunding all belong
//! to the store that sold it, so a client opens that subscription's `manage_url` instead; sending
//! somebody to Allodia's page is how a cancellation quietly does not happen.
//!
//! **Every refusal arrives as a code, never as a sentence.** The service sends its own `message`
//! and this crate never reads it, for the reason [`entitlement.md`](../../../entitlement.md)
//! already gives about grant health: a client that renders a service's own words ships whatever
//! the service happened to say. [`Refusal`] is what a client switches on, and it is what decides
//! whether to say "you have already cancelled" or "that cannot be changed while a charge is being
//! retried".

use serde::Deserialize;

use crate::{
    AccountService, Error, Transport,
    purchase::Plan,
    subscription::{answered, write_request},
};

/// Why the service refused, as a stable code.
///
/// The words a person reads belong to the client, so this carries no text of the service's own. An
/// unrecognised code is [`Refusal::Other`] rather than an error: a client says something general
/// and keeps working, exactly as an unrecognised capability label is kept and never granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Cancelling something already cancelled. Refused rather than quietly doing nothing, so a
    /// client is never left drawing a cancel button that appears to do nothing.
    AlreadyCancelled,
    /// Somebody is already being charged, by **any** source. A second subscription would take
    /// money twice for one plan.
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
    /// A code this version does not know.
    Other(String),
}

impl Refusal {
    /// Read the code out of the service's refusal envelope.
    ///
    /// A body that cannot be read at all is [`Refusal::Other`] with an empty code rather than a
    /// parse error: something refused the call, and reporting "the answer was malformed" would
    /// describe the wrong problem.
    #[must_use]
    pub(crate) fn read(body: &str) -> Self {
        #[derive(Deserialize)]
        struct Envelope {
            data: Option<Code>,
        }
        #[derive(Deserialize)]
        struct Code {
            code: String,
        }
        let code = serde_json::from_str::<Envelope>(body)
            .ok()
            .and_then(|envelope| envelope.data)
            .map(|data| data.code)
            .unwrap_or_default();
        match code.as_str() {
            "already_cancelled" => Self::AlreadyCancelled,
            "already_active" => Self::AlreadyActive,
            "already_on_interval" => Self::AlreadyOnInterval,
            "not_switchable" => Self::NotSwitchable,
            "not_found" => Self::NotFound,
            "unavailable" => Self::Unavailable,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Where to send somebody to pay.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    /// A hosted payment page.
    ///
    /// ⚠️ **Opened in a browser, never in a web view.** It carries the payment-method selection
    /// and the recurring-payment authorisation, which is the same reason RFC 8252 keeps an
    /// authorization request out of one.
    pub checkout_url: String,
}

/// What a cancellation left behind.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancelled {
    /// ISO 8601. What they have paid through, and therefore when access ends.
    ///
    /// **Nothing is refunded**: a month is sold as a month. A client that does not say this before
    /// the confirm button is one whose users think they are getting money back.
    pub end_date: String,
}

/// What switching period actually changed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntervalChange {
    /// The period now in force for the next charge.
    pub interval: String,
    /// What the next charge will be, in minor units.
    pub amount_in_cents: i64,
    /// ISO 8601, and **unchanged by the switch**.
    ///
    /// Worth saying out loud before asking anybody to confirm: no money moves today, nothing is
    /// refunded, and the period already paid for runs on untouched. The switch is otherwise
    /// silent until a date that may be a month away.
    pub next_payment_date: Option<String>,
}

/// What restarting a cancelled subscription did.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resubscribed {
    /// True when it was restarted on the payment authorisation already held: nothing to pay,
    /// nothing to re-enter, and the next charge only once the period already bought runs out.
    pub reactivated: bool,
    /// Where to authorise payment again, when the old authorisation is gone. `None` when
    /// [`reactivated`](Self::reactivated) is true.
    pub checkout_url: Option<String>,
}

impl AccountService {
    /// Get a payment page for a new subscription.
    ///
    /// For the builds that sell through Allodia: Windows, Linux and the direct macOS download. On
    /// iOS and Android the store's own purchase is required, and the result is attached with
    /// `link_store_purchase` instead.
    ///
    /// # Errors
    /// [`Error::Refused`] with [`Refusal::AlreadyActive`] for anybody already paying through any
    /// source, and [`Refusal::Unavailable`] on a deployment with no billing. A client should not
    /// reach either: `actions.can_start_checkout` says so first.
    pub fn start_checkout(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        plan: Plan,
    ) -> Result<Checkout, Error> {
        let body = format!(r#"{{"interval":"{}"}}"#, plan.as_str());
        let response = transport
            .send(&write_request(
                self.base_url(),
                "/subscription/checkout",
                access_token,
                Some(body),
            ))
            .map_err(Error::Transport)?;
        answered(&response)
    }

    /// Stop the recurring charge, keeping everything until the period already paid for runs out.
    ///
    /// Only Allodia's own subscription. One bought through a store is cancelled at that store.
    ///
    /// # Errors
    /// [`Error::Refused`] with [`Refusal::AlreadyCancelled`] when there is nothing to cancel.
    pub fn cancel_subscription(
        &self,
        transport: &dyn Transport,
        access_token: &str,
    ) -> Result<Cancelled, Error> {
        let response = transport
            .send(&write_request(
                self.base_url(),
                "/subscription/cancel",
                access_token,
                None,
            ))
            .map_err(Error::Transport)?;
        answered(&response)
    }

    /// Move between the monthly and the yearly plan.
    ///
    /// **Changes the next charge and nothing else.** No money moves today and nothing is refunded.
    ///
    /// # Errors
    /// [`Error::Refused`] with [`Refusal::NotSwitchable`] for a subscription mid-retry after a
    /// failed charge, or [`Refusal::AlreadyOnInterval`].
    pub fn switch_interval(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        plan: Plan,
    ) -> Result<IntervalChange, Error> {
        let body = format!(r#"{{"interval":"{}"}}"#, plan.as_str());
        let response = transport
            .send(&write_request(
                self.base_url(),
                "/subscription/interval",
                access_token,
                Some(body),
            ))
            .map_err(Error::Transport)?;
        answered(&response)
    }

    /// Start a cancelled subscription again.
    ///
    /// # Errors
    /// [`Error::Refused`] with [`Refusal::AlreadyActive`] for one still running, including one
    /// mid-retry after a failed charge, and for somebody a store is already charging.
    pub fn resubscribe(
        &self,
        transport: &dyn Transport,
        access_token: &str,
    ) -> Result<Resubscribed, Error> {
        let response = transport
            .send(&write_request(
                self.base_url(),
                "/subscription/resubscribe",
                access_token,
                None,
            ))
            .map_err(Error::Transport)?;
        answered(&response)
    }
}
