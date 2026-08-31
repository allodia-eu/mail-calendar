//! What a refusal is allowed to mean, and what an omitted response scope does NOT mean.

use super::{GrantRefusal, GrantedScopes};
use crate::OAuthError;

fn endpoint(error: &str) -> OAuthError {
    OAuthError::Endpoint {
        error: error.to_owned(),
        description: None,
    }
}

#[test]
fn a_refusal_separates_dead_from_under_scoped_from_nothing_learned() {
    assert_eq!(endpoint("invalid_grant").refusal(), GrantRefusal::Dead);
    assert_eq!(
        endpoint("invalid_scope").refusal(),
        GrantRefusal::Underscoped
    );
    // A configuration fault says nothing about the grant, and a caller that treated it as a
    // refusal would sign somebody out over a typo in a client id.
    assert_eq!(
        endpoint("invalid_client").refusal(),
        GrantRefusal::Indeterminate
    );
    assert_eq!(
        OAuthError::Decode("x".to_owned()).refusal(),
        GrantRefusal::Indeterminate
    );
    assert!(!GrantRefusal::Indeterminate.needs_reauth());
}

/// The distinction that decides whether a working account reads as broken.
///
/// RFC 6749 §5.1 makes the response's `scope` optional when it is identical to what was
/// requested, so servers routinely omit it on a grant that was issued in full. Read as "no
/// scopes", that is every feature reporting itself missing on a perfectly good grant.
#[test]
fn an_omitted_response_scope_means_as_requested_not_nothing() {
    let requested = vec!["openid".to_owned(), "mailcal:accounts:read".to_owned()];
    let granted = GrantedScopes::from_response("", &requested);
    assert!(granted.grants("mailcal:accounts:read"));
    assert!(granted.missing(&["mailcal:accounts:read"]).is_empty());
}

#[test]
fn a_named_response_scope_is_what_the_grant_carries_however_much_was_asked_for() {
    let requested = vec!["openid".to_owned(), "mailcal:accounts:read".to_owned()];
    // The shape of a grant that predates a scope: the server issued what it could.
    let granted = GrantedScopes::from_response("openid offline_access", &requested);
    assert!(granted.grants("openid"));
    assert!(!granted.grants("mailcal:accounts:read"));
    assert_eq!(
        granted.missing(&["mailcal:accounts:read", "mailcal:accounts:write"]),
        vec!["mailcal:accounts:read", "mailcal:accounts:write"]
    );
}

#[test]
fn whitespace_between_scopes_is_however_the_server_wrote_it() {
    let granted = GrantedScopes::from_response("  openid \n offline_access  ", &[]);
    assert!(granted.grants("openid"));
    assert!(granted.grants("offline_access"));
    assert_eq!(granted.as_slice().len(), 2);
}
