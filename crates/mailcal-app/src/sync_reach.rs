//! One sync op's bearing on whether an account reached its server, and the verdicts a pass folds
//! those into: reachable or not, and whether its sign-in has expired. The mail pass reads every
//! scope through these; a calendar or contacts pass reads its own ops through them for an account
//! that has no mail to learn from.

use engine_api::SyncError;

use crate::connectivity::{is_signin_expired, is_throttled};

/// One sync op's bearing on whether the account reached its server.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// The op succeeded: the server was reached.
    Reached,
    /// The op failed with a real error (transport, a server fault): the server was not reached.
    Unreachable,
    /// The op failed because the account's stored credential was **refused**
    /// ([`is_signin_expired`]): an expired or revoked OAuth grant. The server answered, so this
    /// is not an outage; it needs a fresh sign-in, which is a different prompt.
    Expired,
    /// The op was refused by a rate limit: the server answered, promptly, and declined to do
    /// the work yet. **Reached, not an outage**: the badge that says "can't reach the server"
    /// would send someone to check a network that is working perfectly.
    Throttled,
    /// The op was skipped because a concurrent sync held the scope ([`ApiError::Busy`]); no
    /// bearing on reachability.
    Busy,
    /// The op failed on a folder the account's folder list no longer holds: deleted or renamed
    /// since the account connected, so its provider names a folder the server does not have.
    /// No bearing on reachability, and not a scope to wait for either.
    Gone,
}

/// Classifies one sync result: success reached the server, a [`ApiError::Busy`] is a
/// concurrent-sync skip (indeterminate), a refused credential is [`Reach::Expired`], and any
/// other error is a real unreachability.
pub(super) fn reach_of<T>(result: &Result<T, SyncError>) -> Reach {
    match result {
        Ok(_) => Reach::Reached,
        Err(err) if err.is_busy() => Reach::Busy,
        // `&ApiError` coerces to `&dyn Error`; the classifier walks its `source()` chain to the
        // typed provider failure and reads the engine's own class.
        Err(err) if is_signin_expired(err) => Reach::Expired,
        Err(err) if is_throttled(err) => Reach::Throttled,
        Err(_) => Reach::Unreachable,
    }
}

/// The same bearing, for one calendar or contacts op: what an account without mail learns about
/// its server, since it has no mail pass to learn it from.
pub(crate) fn reach_of_api<T>(result: &Result<T, engine_api::ApiError>) -> Reach {
    match result {
        Ok(_) => Reach::Reached,
        Err(engine_api::ApiError::Busy) => Reach::Busy,
        Err(err) if is_signin_expired(err) => Reach::Expired,
        Err(err) if is_throttled(err) => Reach::Throttled,
        Err(_) => Reach::Unreachable,
    }
}

/// Folds the pass's op reaches into an account verdict: any success ⇒ reachable
/// (`Some(true)`), else a refused credential ⇒ **reachable** (`Some(true)`: the server answered;
/// the expired-sign-in prompt carries that story instead), else any real failure ⇒ unreachable
/// (`Some(false)`), else all-`Busy` ⇒ indeterminate (`None`, leave the badge as-is). A partial
/// failure (some folders synced, some didn't) still counts as reachable: the server responded.
///
/// [`Reach::Expired`] outranks [`Reach::Unreachable`] deliberately: a dead grant fails *every*
/// op, so a lone transport blip alongside it must not downgrade the account to a generic outage
/// and hide the one prompt that names the actual remedy.
pub(crate) fn reachability(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_reached = false;
    let mut any_expired = false;
    let mut any_unreachable = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            // A throttle is the server answering, so it settles reachability exactly as a
            // success does.
            Reach::Reached | Reach::Throttled => any_reached = true,
            Reach::Expired => any_expired = true,
            Reach::Unreachable => any_unreachable = true,
            Reach::Busy | Reach::Gone => {}
        }
    }
    if any_reached || any_expired {
        Some(true)
    } else if any_unreachable {
        Some(false)
    } else {
        None
    }
}

/// Whether the pass saw a refused credential **and nothing that worked**: the caller raises
/// (`Some(true)`), clears (`Some(false)`) or leaves alone (`None`) the account's expired-sign-in
/// prompt from this.
///
/// One credential serves every scope on an account, so a scope that authenticated disproves an
/// expired one: the refusal came from the server's side and costs the user nothing. Servers do
/// answer this way (a refusal deliberately delayed by ~2s is a rejection, not a timeout) and the
/// prompt is the one thing the user cannot ignore, so it needs evidence nothing else contradicts.
/// The price is that a credential expiring mid-pass is reported one pass later, once nothing
/// succeeds.
///
/// Only an unmixed success clears the prompt, for the same reason read the other way round: a
/// transport failure or a concurrent-sync skip is no evidence the credential works, and a pass
/// that both reached and was refused proves nothing about the credential the user was asked to
/// renew: so neither may retract a prompt they still have to act on.
pub(crate) fn signin_expired(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_reached = false;
    let mut any_expired = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            Reach::Reached => any_reached = true,
            Reach::Expired => any_expired = true,
            // A throttle proves the server answered, but says nothing about the
            // *credential*, which is refused before it is examined. So it neither raises the
            // prompt nor retracts one the user still has to act on.
            Reach::Throttled | Reach::Unreachable | Reach::Busy | Reach::Gone => {}
        }
    }
    match (any_reached, any_expired) {
        (false, true) => Some(true),
        (true, false) => Some(false),
        _ => None,
    }
}

/// Whether this pass was **refused for now** by the account's server: the caller raises
/// (`Some(true)`), clears (`Some(false)`) or leaves alone (`None`) the account's paused notice.
///
/// One refused scope is enough to raise it. A rate limit is an account-wide ceiling, not a
/// folder's, so the folders that did get through this pass got through by being ahead in the
/// queue; saying nothing until every one of them is refused would hide the pause for exactly as
/// long as it takes the account to stop syncing altogether.
///
/// Clearing needs only a pass that was not refused, unlike [`signin_expired`]: the notice states
/// a wait that has since either elapsed or been re-stated, it asks nothing of the user, and
/// leaving a stale one up says mail is not arriving when it is. A pass of only [`Reach::Busy`] and
/// [`Reach::Gone`] proves nothing either way and leaves it alone.
pub(super) fn throttled(list: Reach, folders: impl Iterator<Item = Reach>) -> Option<bool> {
    let mut any_throttled = false;
    let mut any_verdict = false;
    for reach in std::iter::once(list).chain(folders) {
        match reach {
            Reach::Throttled => {
                any_throttled = true;
                any_verdict = true;
            }
            Reach::Reached | Reach::Expired | Reach::Unreachable => any_verdict = true,
            Reach::Busy | Reach::Gone => {}
        }
    }
    any_verdict.then_some(any_throttled)
}

#[cfg(test)]
#[path = "sync_reach_tests.rs"]
mod reachability_tests;
