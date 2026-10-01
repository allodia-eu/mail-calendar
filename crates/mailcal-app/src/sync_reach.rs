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

#[cfg(test)]
mod reachability_tests {
    use super::{Reach, reachability, signin_expired};

    #[test]
    fn a_rate_limited_account_is_reachable_not_offline() {
        // A throttle is the server answering promptly and declining the work. Reading it as
        // an outage puts "can't reach the server" over an account whose network is fine, and
        // sends the user to check their wifi.
        let throttled = || [Reach::Throttled, Reach::Throttled].into_iter();
        assert_eq!(reachability(Reach::Throttled, throttled()), Some(true));
        // And it says nothing about the credential either way: the request was refused
        // before it was examined.
        assert_eq!(signin_expired(Reach::Throttled, throttled()), None);
    }

    #[test]
    fn a_throttle_does_not_retract_a_prompt_the_user_still_has_to_act_on() {
        // The dangerous direction: a pass that is refused for rate and refused for credential
        // must keep the reconnect prompt up, because the throttled scopes never proved the
        // credential works.
        assert_eq!(
            signin_expired(Reach::Expired, [Reach::Throttled].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn a_refused_credential_raises_the_prompt_and_is_not_an_outage() {
        // A dead OAuth grant fails every op. The account is *reachable* (the server answered and
        // said no), so the outage badge stays off and the reconnect prompt carries the story.
        let expired = || [Reach::Expired, Reach::Expired].into_iter();
        assert_eq!(reachability(Reach::Expired, expired()), Some(true));
        assert_eq!(signin_expired(Reach::Expired, expired()), Some(true));
    }

    #[test]
    fn a_refused_credential_outranks_a_transport_failure() {
        // A blip on one folder alongside a dead grant must not downgrade the account to a
        // generic outage, that would hide the one message naming the actual remedy.
        assert_eq!(
            reachability(Reach::Expired, [Reach::Unreachable].into_iter()),
            Some(true),
        );
        assert_eq!(
            signin_expired(Reach::Unreachable, [Reach::Expired].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn a_refusal_beside_a_success_neither_raises_nor_retracts() {
        // One credential serves every scope on an account, so a scope that authenticated in this
        // pass proves the stored credential is still accepted: the refusal is the server's, and
        // must not cost the user a sign-in they do not need.
        assert_eq!(
            signin_expired(Reach::Reached, [Reach::Expired].into_iter()),
            None,
        );
        assert_eq!(
            signin_expired(Reach::Expired, [Reach::Reached].into_iter()),
            None,
        );
        // Nor is a mixed pass evidence to retract a prompt already standing: it proves nothing
        // about the credential the user was asked to renew.
        //
        // A concurrent-sync skip is not a success, so a refusal alongside one still raises; we
        // cannot see whether the sync holding that scope is succeeding.
        assert_eq!(
            signin_expired(Reach::Busy, [Reach::Expired].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn only_a_success_retracts_the_prompt() {
        // Signing in again (or the grant simply working) clears it…
        assert_eq!(
            signin_expired(Reach::Reached, [Reach::Reached].into_iter()),
            Some(false),
        );
        // …but a transport failure or a concurrent-sync skip is no evidence the credential
        // works, so neither may retract a prompt the user still has to act on.
        assert_eq!(signin_expired(Reach::Unreachable, [].into_iter()), None);
        assert_eq!(signin_expired(Reach::Busy, [Reach::Busy].into_iter()), None);
        // A pass with nothing to say about credentials leaves the prompt alone even when it
        // does have something to say about reachability.
        assert_eq!(
            reachability(Reach::Unreachable, [].into_iter()),
            Some(false),
        );
    }

    #[test]
    fn any_success_means_reachable_even_with_a_failed_folder() {
        // The list reached; a folder failing doesn't make the whole account unreachable.
        assert_eq!(
            reachability(Reach::Reached, [Reach::Unreachable].into_iter()),
            Some(true),
        );
        // Or the list failed but a folder reached.
        assert_eq!(
            reachability(Reach::Unreachable, [Reach::Reached].into_iter()),
            Some(true),
        );
    }

    #[test]
    fn every_op_failing_reads_unreachable() {
        assert_eq!(
            reachability(
                Reach::Unreachable,
                [Reach::Unreachable, Reach::Unreachable].into_iter(),
            ),
            Some(false),
        );
    }

    #[test]
    fn a_folder_the_server_no_longer_lists_says_nothing_about_the_account() {
        // Deleted or renamed in another client, it fails on every pass until the account
        // reconnects. That is the folder's absence, not the server's.
        assert_eq!(
            reachability(Reach::Reached, [Reach::Gone].into_iter()),
            Some(true)
        );
        assert_eq!(reachability(Reach::Busy, [Reach::Gone].into_iter()), None);
        assert_eq!(
            signin_expired(Reach::Reached, [Reach::Gone].into_iter()),
            Some(false)
        );
    }

    #[test]
    fn all_busy_is_indeterminate() {
        // A concurrent sync held every scope: no reachability signal, so leave the badge
        // as-is (a concurrent poll + refresh must not falsely mark an account unreachable).
        assert_eq!(reachability(Reach::Busy, [Reach::Busy].into_iter()), None);
    }

    #[test]
    fn busy_never_overrides_a_real_signal() {
        // Busy is ignored, so a real failure alongside busy still reads unreachable…
        assert_eq!(
            reachability(Reach::Busy, [Reach::Unreachable].into_iter()),
            Some(false),
        );
        // …and a success alongside busy still reads reachable.
        assert_eq!(
            reachability(Reach::Busy, [Reach::Reached].into_iter()),
            Some(true),
        );
    }
}
