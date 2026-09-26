//! Reachability classification, proved against the verdicts one pass can produce.
//!
//! Split out of `sync_account.rs` to keep both files under the size limit.

use super::{Reach, reachability, signin_expired, throttled};

#[test]
fn a_rate_limited_account_is_reachable_not_offline() {
    // A throttle is the server answering promptly and declining the work. Reading it as
    // an outage puts "can't reach the server" over an account whose network is fine, and
    // sends the user to check their wifi.
    let refused = || [Reach::Throttled, Reach::Throttled].into_iter();
    assert_eq!(reachability(Reach::Throttled, refused()), Some(true));
    // And it says nothing about the credential either way: the request was refused
    // before it was examined.
    assert_eq!(signin_expired(Reach::Throttled, refused()), None);
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

#[test]
fn one_refused_scope_pauses_the_whole_account() {
    // A rate limit is an account-wide ceiling, not a folder's: the folders that got through
    // this pass got through by being ahead in the queue. Waiting for every one of them to be
    // refused would hide the notice for as long as the account still syncs anything at all.
    assert_eq!(
        throttled(
            Reach::Reached,
            [Reach::Throttled, Reach::Reached].into_iter()
        ),
        Some(true),
    );
    assert_eq!(throttled(Reach::Throttled, [].into_iter()), Some(true));
}

#[test]
fn a_pass_that_was_not_refused_clears_the_notice() {
    // Unlike the expired-sign-in prompt, this needs no unmixed success to retract: the notice
    // asks nothing of the user and states a wait that has since elapsed, so leaving a stale one
    // up says mail is not arriving when it is. Even a pass that failed outright clears it: what
    // the account has then is an outage, which is a different surface with a different remedy.
    assert_eq!(
        throttled(Reach::Reached, [Reach::Reached].into_iter()),
        Some(false)
    );
    assert_eq!(
        throttled(Reach::Unreachable, [Reach::Expired].into_iter()),
        Some(false),
    );
}

#[test]
fn an_all_busy_pass_leaves_the_notice_exactly_as_it_was() {
    // Every scope was held by another pass, so this one looked at nothing and proved nothing.
    // Clearing here would retract a notice on the evidence of a sync that never ran.
    assert_eq!(throttled(Reach::Busy, [Reach::Busy].into_iter()), None);
    assert_eq!(throttled(Reach::Busy, [].into_iter()), None);
}
