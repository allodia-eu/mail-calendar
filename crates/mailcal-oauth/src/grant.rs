//! What a grant carries, and what a refusal of one means.
//!
//! Split from `lib.rs` because it is a vocabulary rather than a flow: every provider's token
//! source reads it, and none of them cares how a request is posted.

use crate::OAuthError;

/// What a token endpoint's refusal means for the stored grant, and therefore what the remedy is.
///
/// The three arms differ in the only way a caller cares about: whether waiting helps, whether
/// signing in again helps, and whether the person is still signed in meanwhile. Classified here,
/// once, so no caller matches on error text: the rule the mail side already follows through
/// `FailureClass::Authentication`, applied to the OAuth layer every provider shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantRefusal {
    /// The grant is **dead**: expired, revoked here, or withdrawn at the provider
    /// (`invalid_grant`). Nothing about it recovers by waiting, and the person is signed out
    /// whether or not this device has noticed.
    Dead,
    /// The grant is **alive but under-scoped**: it was issued without a scope this build asked
    /// for (`invalid_scope`). The person is still signed in and most of the app still works;
    /// re-consenting is what widens it, and waiting never will.
    ///
    /// [`crate::refresh`] no longer names any scope, so this should not arrive on one at all. It
    /// still classified,
    /// because a server may raise it on an exchange, and because a caller that treated it as
    /// transient would retry a request that cannot ever succeed.
    Underscoped,
    /// Anything else: a configuration fault, an unreachable endpoint, a body that could not be
    /// read. Nobody learned anything about the grant, so nothing about it may change.
    Indeterminate,
}

impl GrantRefusal {
    /// Whether signing in again is the remedy; true for a dead grant and an under-scoped one,
    /// false when nothing was learned.
    #[must_use]
    pub const fn needs_reauth(self) -> bool {
        matches!(self, Self::Dead | Self::Underscoped)
    }
}

impl OAuthError {
    /// What this refusal means for the stored grant. See [`GrantRefusal`].
    #[must_use]
    pub fn refusal(&self) -> GrantRefusal {
        match self {
            Self::Endpoint { error, .. } if error == "invalid_grant" => GrantRefusal::Dead,
            Self::Endpoint { error, .. } if error == "invalid_scope" => GrantRefusal::Underscoped,
            _ => GrantRefusal::Indeterminate,
        }
    }

    /// Whether this error means the stored grant is dead and the user must sign in
    /// again (an `invalid_grant` from the endpoint), versus a transient/config fault a
    /// caller might retry or surface differently.
    #[must_use]
    pub fn is_invalid_grant(&self) -> bool {
        self.refusal() == GrantRefusal::Dead
    }
}

/// The scopes a grant actually carries, and what this build wanted.
///
/// A token response names the granted scope only when it **differs** from what was requested
/// (RFC 6749 §5.1), so an empty one means "you got what you asked for" and not "you got
/// nothing": the difference between a working account and one this type would report as broken.
/// Constructing it through [`GrantedScopes::from_response`] is what keeps that distinction in one
/// place instead of at every call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedScopes(Vec<String>);

impl GrantedScopes {
    /// What the grant carries, from a token response's `scope` and the scopes that were asked
    /// for.
    ///
    /// `response_scope` empty ⇒ the request was granted as asked, so `requested` is the answer.
    #[must_use]
    pub fn from_response(response_scope: &str, requested: &[String]) -> Self {
        if response_scope.trim().is_empty() {
            return Self(requested.to_vec());
        }
        Self(
            response_scope
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect(),
        )
    }

    /// What was recorded for a grant a previous launch stored.
    #[must_use]
    pub fn from_stored(scopes: Vec<String>) -> Self {
        Self(scopes)
    }

    /// Whether the grant carries `scope`.
    #[must_use]
    pub fn grants(&self, scope: &str) -> bool {
        self.0.iter().any(|held| held == scope)
    }

    /// The ones in `wanted` this grant does not carry, in the order given.
    ///
    /// Empty means every wanted scope is held, which is the only shape a caller should treat as
    /// "nothing to ask for". A feature names the scope it needs and asks here, so adding a scope
    /// is one line at the feature rather than a new branch in a client.
    #[must_use]
    pub fn missing<'a>(&self, wanted: &[&'a str]) -> Vec<&'a str> {
        wanted
            .iter()
            .copied()
            .filter(|scope| !self.grants(scope))
            .collect()
    }

    /// The scopes themselves, for storing beside the grant.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

#[cfg(test)]
#[path = "grant_tests.rs"]
mod tests;
