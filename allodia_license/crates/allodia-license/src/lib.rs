//! Asking the Allodia account service what an account is entitled to.
//!
//! The rules a client follows are [`entitlement.md`](../../../entitlement.md); this crate is where
//! they live so that four clients cannot each decide them slightly differently. [`Cache`] is the
//! half that matters in practice: what to draw between one answer and the next.
//!
//! It **opens no socket**: the host passes a [`Transport`], because the app already owns TLS and a
//! second provider in the same process is a runtime conflict rather than a convenience. It reads no
//! clock either, taking `now` as Unix seconds. Both make every rule here testable without a network
//! and at an arbitrary time.
//!
//! **Injecting the clock costs no tamper-resistance, because there is none to lose.** A host that
//! would lie about the time is a host that can delete the check above it. This source is
//! published. Nothing here is access control: it decides what a client *draws*, and the server
//! decides what happens, on every request, from the access token. `entitlement.md` → "The client
//! draws; the server decides" is the rule that follows from it.
//!
//! **What the service sends, and what it does not.** A device is never handed a raw entitlement: it
//! would be an unbound bearer that could drain a balance if it leaked. What arrives is derived:
//! which plan, whether it is live, which capabilities to draw, how long to wait before asking
//! again, so there is nothing here to treat as a credential.
//!
//! Signing in is Authorization Code with PKCE against the account service, which is an OAuth 2.0
//! authorization server. That flow is `mailcal-oauth`'s, not this crate's: this one starts at the
//! access token.

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};

mod accounts;
mod cache;
mod feature;
mod projection;
mod reconcile;
mod refresh;
mod signin;
pub use accounts::{
    AccountList, CalDavEndpoint, ConflictWith, DeletedAccount, ImapEndpoint, JmapAuth, Security,
    SmtpEndpoint, SyncedAccount, SyncedConfig,
};
pub use cache::{Cache, GRACE_SECONDS, Outcome, Stored};
pub use feature::Feature;
pub use projection::{NotSyncable, SetupPrefill, to_synced};
pub use reconcile::{Decision, LocalAccount, SyncState, fingerprint, reconcile};
pub use refresh::Refresher;
pub use signin::{
    Endpoints, Identity, Prompt, REDIRECT_HOST, SCOPES, SignIn, SignInError, account_url, api_url,
    available, host,
};

/// The API this crate calls, relative to the account service's root.
///
/// It is also the **audience** a token has to be minted for, which is why it is one constant: the
/// service verifies that a token names the API it is presented to, so a URL and an audience that
/// drifted apart would fail with a `401` that says nothing about which of the two moved.
pub const API_BASE_PATH: &str = "/api/v1";

/// Which HTTP method a [`Request`] uses.
///
/// A closed set rather than a string: the host implements the port, and a method it has not been
/// told about is a request it would have to guess at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Read.
    Get,
    /// Create, where the server mints the identity.
    Post,
    /// Replace a record the caller can already name.
    Put,
    /// Remove one.
    Delete,
}

impl Method {
    /// The name to put on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

/// Where a request should go, and what it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Absolute URL.
    pub url: String,
    /// The access token to present.
    pub bearer: String,
    /// Which method to use.
    pub method: Method,
    /// A JSON body, for the methods that carry one. `None` means send none at all, which is not
    /// the same as sending an empty one, and a `GET` with a body is refused by enough
    /// intermediaries to be worth never producing.
    pub body: Option<String>,
    /// An idempotency key, when the request is one a retry must not duplicate.
    ///
    /// Only `POST` needs it, and only because it is the one write whose identity the server mints:
    /// a create whose response was lost cannot be told from a new account, so without this a retry
    /// on a flaky connection leaves a second record behind. The host sends it as
    /// `Idempotency-Key`.
    pub idempotency_key: Option<String>,
}

impl Request {
    /// A plain authenticated `GET`.
    #[must_use]
    pub fn get(url: impl Into<String>, bearer: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            bearer: bearer.into(),
            method: Method::Get,
            body: None,
            idempotency_key: None,
        }
    }
}

/// What a host hands back after making the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The HTTP status code.
    pub status: u16,
    /// The response body, as received.
    pub body: String,
}

/// Anything that can make an authenticated HTTPS request. The host implements it, so this crate
/// never chooses a TLS provider and never has to agree with the one the app already installed.
pub trait Transport {
    /// Make the request, or say why it could not be made.
    ///
    /// # Errors
    /// Returns the host's own description of a transport failure: a refused connection, a name
    /// that would not resolve, a timeout.
    fn send(&self, request: &Request) -> Result<Response, String>;
}

impl fmt::Debug for dyn Transport + '_ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Transport")
    }
}

/// What went wrong.
///
/// Only [`Error::Unauthorized`] is worth acting on: it means the token needs refreshing, which is
/// `mailcal-oauth`'s job. Everything else is [`Outcome::Unreachable`] as far as the cache is
/// concerned: the client did not learn anything, so it keeps what it had.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The request never reached the service.
    #[error("could not reach the account service: {0}")]
    Transport(String),
    /// The access token has expired or been revoked.
    #[error("the account service refused the access token")]
    Unauthorized,
    /// The service answered, but not with anything this version understands.
    #[error("the account service answered with {status}, which this version cannot read")]
    Unexpected {
        /// The status it answered with.
        status: u16,
    },
    /// The body was not the shape this version expects.
    #[error("the account service's answer could not be read: {0}")]
    Malformed(String),
    /// The record changed elsewhere since this device last read it, carrying what the server holds
    /// now when it said. `None` only when the body could not be read, still a conflict, and still
    /// resolved by re-reading rather than by reporting a broken service.
    #[error("this account was changed elsewhere since it was last read")]
    Conflict(Option<accounts::ConflictWith>),
}

/// A capability a plan turns on.
///
/// The service sends a closed list of labels. An unrecognised one becomes [`Capability::Unknown`]
/// rather than an error: a client that predates a capability must keep working and simply not draw
/// it, which is also why nothing here maps an unknown label to "granted".
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    /// Real-time push on mobile, through the Allodia relay.
    Push,
    /// Send-later on providers whose protocol lacks it.
    SendLater,
    /// Centralized deployment and administration.
    CentralAdmin,
    /// A label this version does not know.
    Unknown(String),
}

impl Capability {
    fn parse(label: &str) -> Self {
        match label {
            "push" => Self::Push,
            "send_later" => Self::SendLater,
            "central_admin" => Self::CentralAdmin,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// What the account may use, as the service last described it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entitlement {
    /// The plan's name, for display. `free` when nothing is active.
    pub plan: String,
    /// Whether a paid plan is live. Anything but an active subscription is `false`.
    pub active: bool,
    /// What to draw. Empty on the free plan.
    pub capabilities: BTreeSet<Capability>,
    /// When the current period ends, as the service wrote it (RFC 3339), if there is one. For
    /// display only: nothing decides anything from it, because the device's clock may disagree.
    pub current_period_end: Option<String>,
}

impl Entitlement {
    /// The free application: every capability absent, and not an error.
    ///
    /// This is what a client shows with no stored answer, past its grace, or on anything it cannot
    /// read. A person whose plan lapsed still has a complete mail and calendar client.
    #[must_use]
    pub fn free() -> Self {
        Self {
            plan: "free".to_owned(),
            active: false,
            capabilities: BTreeSet::new(),
            current_period_end: None,
        }
    }

    /// Whether a capability is granted.
    ///
    /// The list alone is never enough: a lapsed plan can still carry labels, so `active` is checked
    /// on this side too rather than trusted to have been filtered on the other.
    #[must_use]
    pub fn grants(&self, capability: &Capability) -> bool {
        self.active && self.capabilities.contains(capability)
    }
}

/// An entitlement together with how long to wait before asking again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// What the account may use.
    pub entitlement: Entitlement,
    /// How long to wait before asking again, in seconds.
    ///
    /// A **duration**, not a deadline. A device's clock can be wrong by any amount, and an
    /// absolute deadline compared against a skewed one either asks on every launch or never
    /// comes due, both invisible to the device experiencing them.
    pub refresh_after_seconds: i64,
}

// The service is TypeScript and answers in camelCase.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnswerBody {
    plan: String,
    active: bool,
    capabilities: Vec<String>,
    #[serde(default)]
    current_period_end: Option<String>,
    refresh_after_seconds: i64,
}

/// The Allodia account service at a base URL.
#[derive(Debug, Clone)]
pub struct AccountService {
    base_url: String,
}

impl AccountService {
    /// Point at a deployment. The base URL is the site's origin; a trailing slash is fine.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        let base_url = base_url.into();
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    /// The deployment this service points at, without a trailing slash.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Ask what the signed-in account is entitled to.
    ///
    /// The caller holds a live access token; refreshing an expired one is `mailcal-oauth`'s job,
    /// signalled here by [`Error::Unauthorized`].
    ///
    /// # Errors
    /// [`Error::Unauthorized`] when the token needs refreshing; [`Error::Transport`] when the
    /// request never arrived; [`Error::Malformed`] when the answer cannot be read.
    pub fn entitlement(
        &self,
        transport: &dyn Transport,
        access_token: &str,
    ) -> Result<Answer, Error> {
        let response = transport
            .send(&Request::get(
                format!("{}{API_BASE_PATH}/entitlement", self.base_url),
                access_token,
            ))
            .map_err(Error::Transport)?;
        let body = match response.status {
            200..=299 => response.body,
            401 | 403 => return Err(Error::Unauthorized),
            status => return Err(Error::Unexpected { status }),
        };
        let parsed: AnswerBody =
            serde_json::from_str(&body).map_err(|error| Error::Malformed(error.to_string()))?;
        Ok(Answer {
            entitlement: Entitlement {
                plan: parsed.plan,
                active: parsed.active,
                capabilities: parsed
                    .capabilities
                    .iter()
                    .map(|label| Capability::parse(label))
                    .collect(),
                current_period_end: parsed.current_period_end,
            },
            refresh_after_seconds: parsed.refresh_after_seconds,
        })
    }
}

#[cfg(test)]
mod tests;
