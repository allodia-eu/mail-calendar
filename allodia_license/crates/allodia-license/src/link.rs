//! Attaching a store purchase to the account that paid for it.
//!
//! `POST /subscription/store` takes **one identifier** and nothing else: Apple's transaction id or
//! Play's purchase token. Every fact about the subscription is then read back from the store by
//! the service, so nothing a device claims about what it bought is believed, and there is no
//! signed payload for this repository to carry or to mistakenly parse.
//!
//! **What the device still owes the store afterwards is not the same on both.** The service
//! acknowledges a Play purchase itself, which is what Play requires within three days or it
//! refunds it. A StoreKit transaction can only be finished by the device that holds it, so on
//! Apple that stays a client's job, and the order it happens in is the rule everything here exists
//! for:
//!
//! **A transaction is never finished with the store until the service has attached it.** StoreKit
//! re-delivers an unfinished transaction at every launch, for as long as it takes, and that is the
//! only copy of a purchase somebody paid for.
//!
//! **The distinction from `entitlement.md` applies again, for the same reason.** "The service
//! refused this purchase" and "I could not ask" are different answers, and collapsing them either
//! discards a real purchase or retries a dead one forever.
//!
//! Nothing here reads a clock or opens a socket; `now` arrives as Unix seconds and the request
//! goes out through the host's [`Transport`](crate::Transport), as everywhere else in this crate.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AccountService, Error, Method, Request, Response, Transport, purchase::Store};

/// Where a store purchase is attached, relative to the account service's root.
const LINK_PATH: &str = "/subscription/store";

/// A purchase a store says this person owns.
///
/// **It is deliberately not serializable, so it cannot be written down.** The identifier is what
/// claims a subscription: presenting one attaches it to whoever is signed in, which is exactly
/// what `owned_by_another_account` reports happening to somebody else. The store hands it back at
/// every launch, so a device has no reason to keep a copy.
#[derive(Clone, PartialEq, Eq)]
pub struct StorePurchase {
    /// Which shop sold it, so the service knows whose API to read it back from.
    pub store: Store,
    /// The identifier the service takes: Apple's transaction id, Play's purchase token.
    ///
    /// Two jobs. It deduplicates the ledger, because StoreKit re-delivers the same unfinished
    /// transaction at every launch and Play returns the same token from every query; and it is
    /// what makes the call idempotent, because re-sending one changes nothing.
    pub purchase: String,
    /// When the store says the purchase was made, in Unix seconds.
    pub purchased_at: i64,
}

/// Redacted, on purpose.
///
/// The identifier claims a subscription, so it is a credential in the way that matters and
/// [`logging.md`](../../../docs/logging.md) keeps those out of the log. What a diagnostic log gets
/// is which store and when, which is enough to follow a purchase without carrying the claim.
impl fmt::Debug for StorePurchase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorePurchase")
            .field("store", &self.store)
            .field("purchase", &"<redacted>")
            .field("purchased_at", &self.purchased_at)
            .finish()
    }
}

/// What the account service said about a purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    /// Attached to the signed-in account. On Apple the client finishes the transaction; on Play
    /// the service has already acknowledged it and there is nothing left to do.
    Linked,
    /// `owned_by_another_account`: already attached to a **different** Allodia account.
    ///
    /// What restoring purchases onto a second Allodia account looks like. Retrying cannot help,
    /// and on Apple the transaction is finished anyway: it is a real purchase doing its job
    /// elsewhere, and leaving it unfinished makes StoreKit re-deliver it at every launch for the
    /// rest of the install's life. What the person needs is to be told which of the two happened,
    /// which is why this is not folded into a generic failure.
    ClaimedElsewhere,
    /// Nothing was learned: an outage, a captive portal, an expired token, a deployment with no
    /// billing configured, or a purchase the store does not recognise **yet**. The purchase stays,
    /// and nothing is finished.
    ///
    /// `unknown_purchase` belongs here rather than in a terminal state of its own, and that is a
    /// decision rather than an oversight: a transaction made seconds ago is routinely not yet
    /// queryable at the store, so the first answer about a real purchase can be this one. Retrying
    /// costs one request an hour once the backoff has opened up; concluding from it would throw
    /// away a purchase somebody had just made.
    Deferred,
}

/// A purchase waiting to be attached, and how that has gone so far this session.
///
/// **Nothing here outlives the process, and nothing needs to.** The store is the durable copy: it
/// offers an unfinished purchase again at every launch, and it says when the purchase was made, so
/// a relaunch rebuilds this from the store rather than from anything the device wrote down. What
/// is lost is the attempt count, which only paces retries inside one session.
///
/// That is worth more than the persistence it replaces. There is no ledger file to corrupt, no
/// preferences key to migrate, and nothing for five clients to implement; and the one thing that
/// genuinely must survive a relaunch, how long somebody has been waiting for a purchase they paid
/// for, comes from the store's own timestamp rather than from a count that resets every time the
/// app is killed.
#[derive(Clone, PartialEq, Eq)]
pub struct Pending {
    /// Which shop sold it.
    pub store: Store,
    /// The identifier the service takes. Redacted from `Debug` for the reason
    /// [`StorePurchase`] is.
    pub purchase: String,
    /// When the **store** says the purchase was made, in Unix seconds.
    ///
    /// StoreKit's purchase date, Play's purchase time. Taken from the store rather than from when
    /// this device first noticed, so a reinstall or a relaunch cannot make an old unattached
    /// purchase look new.
    pub purchased_at: i64,
    /// When it was last attempted, in Unix seconds. `None` before the first attempt, which is what
    /// makes a fresh purchase due immediately.
    pub last_attempt_at: Option<i64>,
    /// How many attempts have been made this session.
    pub attempts: u32,
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pending")
            .field("store", &self.store)
            .field("purchase", &"<redacted>")
            .field("purchased_at", &self.purchased_at)
            .field("last_attempt_at", &self.last_attempt_at)
            .field("attempts", &self.attempts)
            .finish()
    }
}

/// The shortest wait between attempts, in seconds.
const BACKOFF_FLOOR_SECONDS: i64 = 60;

/// The longest wait between attempts, in seconds.
///
/// An hour, and capped rather than allowed to keep doubling, because Play refunds an
/// unacknowledged purchase after three days and the acknowledgement happens on the far side of
/// this call. A backoff that grew past that would turn a service having a bad afternoon into a
/// refund the person did not ask for.
const BACKOFF_CEILING_SECONDS: i64 = 60 * 60;

/// How long after a purchase it is worth telling the person it has not gone through.
///
/// An hour. Before that, saying anything would be reporting a slow network as a problem with their
/// payment. Measured from the **store's** purchase time, so it survives the app being killed,
/// which on a phone is the ordinary case rather than the exception.
const STUCK_AFTER_SECONDS: i64 = 60 * 60;

impl Pending {
    /// How long to wait after the last attempt before trying again.
    ///
    /// Doubling from a minute to an hour. The first attempt has no wait at all, which is what
    /// carries a purchase straight through in the ordinary case.
    #[must_use]
    pub fn backoff_seconds(&self) -> i64 {
        BACKOFF_FLOOR_SECONDS
            .saturating_mul(1_i64 << self.attempts.saturating_sub(1).min(6))
            .min(BACKOFF_CEILING_SECONDS)
    }

    /// Whether it is time to try this one again.
    #[must_use]
    pub fn is_due(&self, now: i64) -> bool {
        match self.last_attempt_at {
            None => true,
            Some(last) => now.saturating_sub(last) >= self.backoff_seconds(),
        }
    }

    /// Whether this has been waiting long enough to be worth telling the person about.
    ///
    /// Money was taken and nothing was granted. Saying nothing at all is the one response that
    /// leaves them with no way to find out.
    #[must_use]
    pub fn is_stuck(&self, now: i64) -> bool {
        now.saturating_sub(self.purchased_at) >= STUCK_AFTER_SECONDS
    }
}

/// What a client owes the store once the ledger has folded in an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// Done with, as far as the service is concerned.
    ///
    /// On Apple that means finishing the transaction, which only the device can do. On Play it
    /// means nothing at all: the service acknowledged the purchase when it attached it.
    Settled,
    /// Leave it alone. It stays in the ledger and will come round again.
    Keep,
}

/// Every store purchase this process has not yet had attached.
///
/// Held for the life of the process and rebuilt from the store at the next launch. See [`Pending`]
/// for why nothing here is written down.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pending: Vec<Pending>,
}

impl Ledger {
    /// Start from what is already known, or from nothing.
    #[must_use]
    pub fn restore(pending: Vec<Pending>) -> Self {
        Self { pending }
    }

    /// Everything still waiting.
    #[must_use]
    pub fn pending(&self) -> &[Pending] {
        &self.pending
    }

    /// Bring the ledger into step with everything the store is reporting now.
    ///
    /// **The store is the authority, so this is one operation rather than an add and a remove.**
    /// A caller hands over every purchase the store currently says this person owns, and what
    /// comes back is a ledger describing exactly that: purchases it had not seen are taken up,
    /// ones the store no longer reports are dropped, and the retry pacing of the rest is left
    /// alone.
    ///
    /// The deduplication is the load-bearing half, because this is called with the same purchase
    /// over and over: StoreKit re-delivers every unfinished transaction at launch and again
    /// through its updates stream, and Play returns it from every `queryPurchasesAsync`.
    ///
    /// A purchase that needs no attaching is ignored. Allodia's own checkout is billed against the
    /// account already, so there is nothing for a device to carry, and a caller that passed one has
    /// confused the two routes rather than found a third.
    pub fn sync(&mut self, reported: &[StorePurchase]) {
        self.pending.retain(|entry| {
            reported
                .iter()
                .any(|purchase| purchase.purchase == entry.purchase)
        });
        for purchase in reported
            .iter()
            .filter(|purchase| purchase.store.needs_attaching())
        {
            if self
                .pending
                .iter()
                .any(|entry| entry.purchase == purchase.purchase)
            {
                continue;
            }
            self.pending.push(Pending {
                store: purchase.store,
                purchase: purchase.purchase.clone(),
                purchased_at: purchase.purchased_at,
                last_attempt_at: None,
                attempts: 0,
            });
        }
    }

    /// Which purchases are worth attempting right now, oldest first.
    ///
    /// Oldest first because on Play the oldest is the closest to being refunded.
    #[must_use]
    pub fn due(&self, now: i64) -> Vec<&Pending> {
        let mut due: Vec<&Pending> = self
            .pending
            .iter()
            .filter(|entry| entry.is_due(now))
            .collect();
        due.sort_by_key(|entry| entry.purchased_at);
        due
    }

    /// Whether any purchase has been waiting long enough to tell the person about.
    #[must_use]
    pub fn has_stuck(&self, now: i64) -> bool {
        self.pending.iter().any(|entry| entry.is_stuck(now))
    }

    /// Fold in what the service said about one purchase, and say what the store is still owed.
    ///
    /// A purchase the ledger does not hold settles as [`Settled::Settled`] rather than as nothing:
    /// it is one attached on an earlier run whose store call was lost, and StoreKit will keep
    /// offering it until somebody finishes it.
    pub fn apply(&mut self, purchase: &str, outcome: LinkOutcome, now: i64) -> Settled {
        match outcome {
            LinkOutcome::Linked | LinkOutcome::ClaimedElsewhere => {
                self.pending.retain(|entry| entry.purchase != purchase);
                Settled::Settled
            }
            LinkOutcome::Deferred => {
                if let Some(entry) = self
                    .pending
                    .iter_mut()
                    .find(|entry| entry.purchase == purchase)
                {
                    entry.attempts = entry.attempts.saturating_add(1);
                    entry.last_attempt_at = Some(now);
                }
                Settled::Keep
            }
        }
    }
}

// The service is TypeScript and answers in camelCase.
#[derive(Serialize)]
struct LinkBody<'a> {
    store: &'a str,
    purchase: &'a str,
}

/// The service's refusal envelope. `data.code` is the stable reason; `message` is the service's
/// own sentence and is **never** shown, because the words a person reads belong to the client.
#[derive(Deserialize)]
struct Refusal {
    data: Option<RefusalData>,
}

#[derive(Deserialize)]
struct RefusalData {
    code: String,
}

/// The one refusal code a client acts differently on.
const OWNED_BY_ANOTHER_ACCOUNT: &str = "owned_by_another_account";

impl AccountService {
    /// Attach a store purchase to whoever the access token belongs to.
    ///
    /// **The account is the token's**, never anything the body names, which is why the body
    /// carries a store and an identifier and nothing else. A device that could say which account
    /// to attach to could attach somebody else's purchase to itself.
    ///
    /// Idempotent at the service, so a call whose response was lost costs a repeat and nothing
    /// more.
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing; [`Error::Transport`] when the
    /// request never arrived. Both are [`LinkOutcome::Deferred`] as far as the ledger is concerned:
    /// nothing was learned, so nothing is finished.
    pub fn link_store_purchase(
        &self,
        transport: &dyn Transport,
        access_token: &str,
        purchase: &StorePurchase,
    ) -> Result<LinkOutcome, Error> {
        let body = serde_json::to_string(&LinkBody {
            store: purchase.store.as_str(),
            purchase: &purchase.purchase,
        })
        .map_err(|error| Error::Malformed(error.to_string()))?;
        let response = transport
            .send(&Request {
                url: format!("{}{}{LINK_PATH}", self.base_url(), crate::API_BASE_PATH),
                bearer: access_token.to_owned(),
                method: Method::Post,
                body: Some(body),
                // The purchase's own identity. The service is idempotent on it already; sending it
                // costs nothing and means a retry cannot be told from the first attempt by
                // anything in between either.
                idempotency_key: Some(purchase.purchase.clone()),
            })
            .map_err(Error::Transport)?;
        Ok(interpret(&response))
    }
}

/// What an answer means for a purchase.
///
/// **Read from the refusal's `data.code`, not from the status.** The service returns `404` for
/// both `unknown_purchase` and an ordinary missing record, and a `409` could later carry a code
/// that is not this one, so the status alone decides too much. Everything that is not a success
/// and not this one code defers: concluding from a status or a code a later service invented is
/// the one mistake here that cannot be undone.
fn interpret(response: &Response) -> LinkOutcome {
    if let 200..=299 = response.status {
        return LinkOutcome::Linked;
    }
    let claimed = serde_json::from_str::<Refusal>(&response.body)
        .ok()
        .and_then(|refusal| refusal.data)
        .is_some_and(|data| data.code == OWNED_BY_ANOTHER_ACCOUNT);
    if claimed {
        LinkOutcome::ClaimedElsewhere
    } else {
        LinkOutcome::Deferred
    }
}

#[cfg(test)]
#[path = "link_tests.rs"]
mod tests;
