//! What the app is allowed to conclude about an Allodia sign-in, and (mostly) what it is not.

use allodia_license::Feature;
use mailcal_oauth::GrantRefusal;

use super::{AllodiaGrantHealth, grant_permits, health_from_scopes};

fn scopes(list: &[&str]) -> Option<Vec<String>> {
    Some(list.iter().map(|s| (*s).to_owned()).collect())
}

fn every_scope() -> Option<Vec<String>> {
    Some(
        Feature::ALL
            .iter()
            .map(|feature| feature.scope().to_owned())
            .collect(),
    )
}

#[test]
fn a_refused_grant_says_signed_out_and_a_narrow_one_says_sign_in_again() {
    assert_eq!(
        AllodiaGrantHealth::from_refusal(GrantRefusal::Dead),
        Some(AllodiaGrantHealth::SignedOut)
    );
    assert_eq!(
        AllodiaGrantHealth::from_refusal(GrantRefusal::Underscoped),
        Some(AllodiaGrantHealth::NeedsReauth)
    );
}

/// The arm that keeps a bad afternoon at the service from signing anybody out.
///
/// It is the same rule the entitlement contract states: an unreachable service changes nothing,
/// while an explicit refusal takes effect at once, and the reason there is no `Unreachable`
/// health to record.
#[test]
fn a_failure_that_says_nothing_about_the_grant_records_nothing() {
    assert_eq!(
        AllodiaGrantHealth::from_refusal(GrantRefusal::Indeterminate),
        None
    );
}

#[test]
fn a_grant_carrying_every_feature_scope_is_healthy() {
    assert_eq!(
        health_from_scopes(every_scope().as_ref()),
        Some(AllodiaGrantHealth::Ok)
    );
    for feature in Feature::ALL {
        assert!(grant_permits(every_scope().as_ref(), *feature));
    }
}

/// The live case: a grant issued before `mailcal:accounts:read` existed.
#[test]
fn a_grant_predating_a_scope_needs_signing_in_again_and_permits_only_what_it_has() {
    let old = scopes(&["openid", "offline_access", "mailcal:entitlement:read"]);
    assert_eq!(
        health_from_scopes(old.as_ref()),
        Some(AllodiaGrantHealth::NeedsReauth)
    );
    assert!(grant_permits(old.as_ref(), Feature::Entitlement));
    assert!(!grant_permits(old.as_ref(), Feature::ReadAccounts));
    assert!(!grant_permits(old.as_ref(), Feature::WriteAccounts));
}

/// A grant stored before this field existed, which is every grant in the wild on the day it ships.
///
/// Read as "carries nothing" it would prompt every signed-in person the moment they updated, and
/// withhold every feature from them until they did; on grants that are, for the most part,
/// perfectly good. Not knowing is not evidence: nothing is concluded, and the request itself
/// remains the authority.
#[test]
fn an_unrecorded_scope_set_concludes_nothing_and_withholds_nothing() {
    assert_eq!(health_from_scopes(None), None);
    for feature in Feature::ALL {
        assert!(
            grant_permits(None, *feature),
            "an unknown scope set must not withhold {feature:?}"
        );
    }
}

/// Adding a scope must not need a new branch anywhere: the whole point of `Feature`.
#[test]
fn every_feature_scope_is_one_the_sign_in_actually_asks_for() {
    for feature in Feature::ALL {
        assert!(
            allodia_license::SCOPES.contains(&feature.scope()),
            "{feature:?} needs {} but no sign-in ever asks for it, so the prompt it raises \
             could never be satisfied",
            feature.scope()
        );
    }
}
