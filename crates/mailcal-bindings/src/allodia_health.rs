//! Whether the Allodia sign-in this device holds can still do what the app asks of it.
//!
//! A grant is not simply good or bad. It can be **dead**; revoked here, or the account removed on
//! another device, and it can be **alive but narrower than this build needs**, because it was
//! issued before a scope existed. Those are different sentences to a person: one says they are
//! signed out, the other says they are signed in and one feature is asleep. They were the same
//! `MailcalError::Connect(String)` until this module existed, so the only thing a client could do
//! was print it, and a screenshot of `oauth endpoint error: invalid_scope; unable to issue scope
//! mailcal:accounts:read` is what that looked like.
//!
//! **An unreachable service is not a third health.** It is the absence of one: nobody learned
//! anything, so nothing here changes: the same rule the entitlement contract states for a stored
//! answer, and the one `docs/sync-progress.md` states for a pass nobody awaited. That is why
//! [`AllodiaGrantHealth`] has no `Unreachable` arm; a caller that could not ask simply does not
//! record.

#[cfg(feature = "allodia-license")]
use allodia_license::Feature;
use mailcal_oauth::GrantRefusal;
#[cfg(feature = "allodia-license")]
use mailcal_oauth::GrantedScopes;

/// What this device knows about its Allodia sign-in.
///
/// Recorded from evidence: a refused request, or a granted scope set read back from the service;
/// and never guessed. A client draws from this rather than from any error text.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllodiaGrantHealth {
    /// Nothing is known to be wrong. The starting state, and where a successful pass puts it back.
    ///
    /// The default, deliberately: a client that has not asked yet knows nothing, and knowing
    /// nothing must draw nothing rather than a prompt nobody has earned.
    #[default]
    Ok,
    /// Still signed in, but the grant predates a scope this build needs, so a feature cannot run.
    ///
    /// The remedy is signing in again, which re-runs the ordinary flow and asks for the full
    /// current set. Nothing else about the account is affected, and nothing is lost by ignoring
    /// it, which is why a client draws this as an offer and not as an error.
    NeedsReauth,
    /// The service refused the grant outright: revoked here, expired, or the account removed on
    /// another device. The person is signed out whether or not this device had noticed.
    SignedOut,
}

impl AllodiaGrantHealth {
    /// What a refusal from the token endpoint means for the sign-in, or `None` when it means
    /// nothing about it at all.
    ///
    /// `None` is the important arm. A configuration fault, a dead network, a body that could not
    /// be read; none of them are evidence about the grant, and recording one as a health would
    /// sign somebody out over a bad afternoon at the service.
    #[must_use]
    #[cfg_attr(
        not(feature = "allodia-license"),
        expect(
            dead_code,
            reason = "only a build that can sign in ever classifies a refusal"
        )
    )]
    pub(crate) fn from_refusal(refusal: GrantRefusal) -> Option<Self> {
        match refusal {
            GrantRefusal::Dead => Some(Self::SignedOut),
            GrantRefusal::Underscoped => Some(Self::NeedsReauth),
            GrantRefusal::Indeterminate => None,
        }
    }
}

/// What the recorded scope set says about a grant, before any request is made.
///
/// The fast path: a feature that needs a scope the grant does not carry can be reported without
/// the round trip that would fail. `None` for a grant whose scopes were never recorded; every one
/// stored by a build predating the field; because "not known" is not evidence of anything, and
/// treating it as "carries nothing" would prompt every existing user on sight.
#[must_use]
#[cfg(feature = "allodia-license")]
pub(crate) fn health_from_scopes(granted: Option<&Vec<String>>) -> Option<AllodiaGrantHealth> {
    let granted = GrantedScopes::from_stored(granted?.clone());
    let wanted: Vec<&str> = Feature::ALL.iter().map(|feature| feature.scope()).collect();
    if granted.missing(&wanted).is_empty() {
        Some(AllodiaGrantHealth::Ok)
    } else {
        Some(AllodiaGrantHealth::NeedsReauth)
    }
}

/// Whether the grant permits `feature`, as far as this device knows.
///
/// A grant with no recorded scopes answers `true`: not knowing is not a reason to withhold
/// something the person may well be entitled to, and the request itself is the authority. The
/// recorded set only ever saves a round trip that was going to fail.
#[must_use]
#[cfg(feature = "allodia-license")]
pub(crate) fn grant_permits(granted: Option<&Vec<String>>, feature: Feature) -> bool {
    granted.is_none_or(|scopes| GrantedScopes::from_stored(scopes.clone()).grants(feature.scope()))
}

#[cfg(all(test, feature = "allodia-license"))]
#[path = "allodia_health_tests.rs"]
mod tests;
