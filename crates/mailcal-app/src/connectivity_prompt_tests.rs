//! Tests for the three prompts an account can carry beyond an outage badge: the mail write/send
//! re-consent gap, the calendar re-consent gap, and the expired sign-in. Each has its own remedy,
//! so each is classified structurally (on the engine's failure class, never on wording) and each
//! has its own raise/retract rule: the sign-in prompt's is the strictest, since it is the one a
//! user cannot ignore. Split from `connectivity_tests.rs`, which keeps the offline short-circuit
//! and the per-account outage badges, to stay under the size limit.

use std::sync::{Arc, Mutex};

use engine_api::AccountId;
use engine_provider::ProviderError;
use fakes::{FakeProvider, account, account_with, app};

use super::{Intent, Surface};
use crate::connectivity::{is_graph_permission_denied, is_signin_expired};

// The shared fixtures are also included by `tests.rs`; each test file compiles them into its
// own module tree, which is intentional (they share no state); silence the duplicate-load lint.
#[allow(clippy::duplicate_mod)]
#[path = "tests_fakes.rs"]
mod fakes;

/// Mirrors how the outbox's `ApiError` nests a provider failure (`Sync(Provider(..))`), so the
/// classifier's `source()` walk is exercised: not just a bare provider error at depth 0.
#[derive(Debug)]
struct Wrapped(ProviderError);

impl std::fmt::Display for Wrapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "outer: {}", self.0)
    }
}

impl std::error::Error for Wrapped {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn graph_insufficient_permissions_is_detected_structurally() {
    // The real refusal Graph returns on a send/edit when the grant lacks `Mail.ReadWrite`/
    // `Mail.Send`, nested one level down exactly as the outbox's `ApiError` wraps a provider error.
    let denied = Wrapped(ProviderError::permanent(
        "Graph HTTP 403 (code Some(\"ErrorAccessDenied\")): {\"error\":{\"code\":\
         \"ErrorAccessDenied\",\"message\":\"Access is denied. Check credentials and try again.\"}}",
    ));
    assert!(
        is_graph_permission_denied(&denied),
        "a nested 403 ErrorAccessDenied flags the account for re-consent",
    );

    // A bare provider error (depth 0) is matched too.
    assert!(is_graph_permission_denied(&ProviderError::permanent(
        "Graph HTTP 403 (code Some(\"ErrorAccessDenied\")): access is denied",
    )));

    // A *different* 403 (the idempotent re-delete of an already-purged message) must NOT be
    // mistaken for a permission gap (it is an outbox concern, not a re-consent one). This is why
    // the mail side matches the `ErrorAccessDenied` code specifically, not a bare `403`.
    let redelete = Wrapped(ProviderError::permanent(
        "Graph HTTP 403 (code Some(\"ErrorCannotDeleteObject\")): {\"error\":{\"code\":\
         \"ErrorCannotDeleteObject\"}}",
    ));
    assert!(
        !is_graph_permission_denied(&redelete),
        "a non-authorization 403 does not flag for re-consent",
    );

    // A transient server error is not a permission gap (retry, not re-consent).
    let transient = Wrapped(ProviderError::retryable(
        "Graph HTTP 500 (code Some(\"ErrorInternalServerError\")): try again",
    ));
    assert!(!is_graph_permission_denied(&transient));
}

#[test]
fn a_refused_credential_is_detected_structurally() {
    // The real Google refusal behind the "Can't reach this account's server" badge this replaces:
    // the refresh token is gone, so the token endpoint answers `invalid_grant`. Nested one level
    // down, exactly as an `ApiError` wraps a provider error.
    let revoked = Wrapped(ProviderError::authentication(
        "oauth endpoint error: invalid_grant; Token has been expired or revoked.",
    ));
    assert!(
        is_signin_expired(&revoked),
        "a nested authentication failure asks the user to sign in again",
    );

    // A bare provider error (depth 0) is matched too.
    assert!(is_signin_expired(&ProviderError::authentication(
        "IMAP LOGIN failed: authentication failed",
    )));

    // Matched on the engine's *class*, not on wording: so a permanent failure that merely talks
    // about credentials is not mistaken for a dead grant, and no retryable failure ever is.
    assert!(
        !is_signin_expired(&Wrapped(ProviderError::permanent(
            "Graph HTTP 403 (code Some(\"ErrorAccessDenied\")): Access is denied. Check \
             credentials and try again.",
        ))),
        "a missing *scope* is a different prompt from a dead sign-in",
    );
    assert!(!is_signin_expired(&Wrapped(ProviderError::retryable(
        "JMAP HTTP 503: authentication service unavailable",
    ))));

    // A failure that never came from a provider at all (a store error) is neither.
    assert!(!is_signin_expired(&Wrapped(ProviderError::conflict(
        "UIDVALIDITY changed",
    ))));
}

#[tokio::test]
async fn the_mail_reauth_prompt_sets_clears_and_signals_only_on_change() {
    // Drives the mail write/send re-consent flag directly: raising it lists the account and
    // signals, re-raising is silent, it survives going offline (a standing permission gap, not an
    // outage), and clearing drops it + signals.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("b", FakeProvider::new())], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    app.note_mail_reauth_required(&id);
    assert_eq!(
        app.connectivity().mail_reauth_accounts,
        vec!["b".to_string()]
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));

    surfaces.lock().unwrap().clear();
    app.note_mail_reauth_required(&id);
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "re-raising an already-flagged account signals nothing",
    );

    // Unlike an outage badge, a permission gap is NOT suppressed while the device is offline.
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    assert!(app.connectivity().offline);
    assert_eq!(
        app.connectivity().mail_reauth_accounts,
        vec!["b".to_string()],
        "the permission gap is real regardless of connectivity",
    );

    app.dispatch(Intent::ReportNetworkReachable(true)).await;
    surfaces.lock().unwrap().clear();
    app.clear_mail_reauth_required(&id);
    assert!(
        app.connectivity().mail_reauth_accounts.is_empty(),
        "clearing (a re-auth, a successful send, or removal) drops the prompt",
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
}

#[tokio::test]
async fn an_expired_signin_replaces_the_outage_badge_rather_than_joining_it() {
    // The bug this fixes: a revoked Google grant rendered as "Can't reach this account's server."
    // The server *was* reached (it refused the credential) so the account must carry the
    // reconnect prompt and must NOT also be badged unreachable, or the two contradict each other.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("b", FakeProvider::new())], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    // A failing pass marks the account unreachable, as any sync failure would.
    app.set_account_reachable(&id, false);
    assert_eq!(
        app.connectivity().unreachable_accounts,
        vec!["b".to_string()]
    );

    app.note_signin_expired(&id);
    let snapshot = app.connectivity();
    assert_eq!(snapshot.signin_expired_accounts, vec!["b".to_string()]);
    assert!(
        snapshot.unreachable_accounts.is_empty(),
        "a dead sign-in owns the account's message; it is not also an outage",
    );
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));

    surfaces.lock().unwrap().clear();
    app.note_signin_expired(&id);
    assert!(
        surfaces.lock().unwrap().is_empty(),
        "re-raising an already-flagged account signals nothing",
    );

    surfaces.lock().unwrap().clear();
    app.clear_signin_expired(&id);
    let snapshot = app.connectivity();
    assert!(snapshot.signin_expired_accounts.is_empty());
    assert!(surfaces.lock().unwrap().contains(&Surface::Connectivity));
    assert_eq!(
        snapshot.unreachable_accounts,
        vec!["b".to_string()],
        "the underlying outage state was masked, not discarded",
    );

    // Like the other re-consent prompts it survives going offline: signing in again is the remedy
    // whether or not the device has a network right now. Left until last because coming back
    // online refreshes, and a refresh that reaches the server clears the prompt by itself.
    app.note_signin_expired(&id);
    app.dispatch(Intent::ReportNetworkReachable(false)).await;
    let snapshot = app.connectivity();
    assert!(snapshot.offline);
    assert_eq!(snapshot.signin_expired_accounts, vec!["b".to_string()]);
}

#[tokio::test]
async fn one_refused_folder_beside_a_working_one_never_asks_for_a_new_signin() {
    // A real server answers this: five refusals in twelve hours on an account whose sibling
    // folders authenticated in the same second, each delayed ~2s (Dovecot's `auth_failure_delay`,
    // so a deliberate rejection, not a timeout). One credential serves every scope, so a scope
    // that authenticated proves the stored one is accepted, asking the user to sign in again
    // would be asking them to fix the server's mind.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account_with(
            "b",
            vec![
                FakeProvider::new(),
                FakeProvider::folder("b2", Vec::new()).refusing_signin(),
            ],
        )],
        &surfaces,
    );
    let id = AccountId::try_from("b").unwrap();

    app.dispatch(Intent::RefreshMail).await;
    assert!(
        app.connectivity().signin_expired_accounts.is_empty(),
        "a refusal beside a success must not raise the prompt",
    );
    assert!(
        app.connectivity().unreachable_accounts.is_empty(),
        "nor is a refused scope an outage: the rest of the account synced",
    );

    // Nor may a mixed pass retract a prompt the user still has to act on: it proves nothing about
    // the credential they were asked to renew.
    app.note_signin_expired(&id);
    app.dispatch(Intent::RefreshMail).await;
    assert_eq!(
        app.connectivity().signin_expired_accounts,
        vec!["b".to_string()],
        "a standing prompt survives a pass that both reached and was refused",
    );
}

#[tokio::test]
async fn a_refusal_with_nothing_else_working_does_ask_for_a_new_signin() {
    // The other side of the rule, through the real wiring: when no scope on the account
    // authenticated, the refusal is all the evidence there is, and the user must be told.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(
        vec![account_with(
            "b",
            vec![FakeProvider::new().refusing_signin()],
        )],
        &surfaces,
    );

    app.dispatch(Intent::RefreshMail).await;
    let snapshot = app.connectivity();
    assert_eq!(snapshot.signin_expired_accounts, vec!["b".to_string()]);
    assert!(
        snapshot.unreachable_accounts.is_empty(),
        "the server answered; it refused the credential, which is not an outage",
    );
}

#[tokio::test]
async fn a_successful_sync_retracts_the_expired_signin_prompt() {
    // The other half of the loop: the prompt is not sticky. Once the account syncs again; the
    // user signed in, or the grant simply worked: the pass's own verdict clears it, with no host
    // involvement. Proven through a real refresh rather than the setter, so the wiring from
    // `sync_account_providers` through `apply_signin_expired` is what is under test.
    let surfaces = Arc::new(Mutex::new(Vec::new()));
    let app = app(vec![account("b", FakeProvider::new())], &surfaces);
    let id = AccountId::try_from("b").unwrap();

    app.note_signin_expired(&id);
    assert_eq!(
        app.connectivity().signin_expired_accounts,
        vec!["b".to_string()],
    );

    app.dispatch(Intent::RefreshMail).await;
    assert!(
        app.connectivity().signin_expired_accounts.is_empty(),
        "a sync that reached the server proves the credential works; retract the prompt",
    );
}
