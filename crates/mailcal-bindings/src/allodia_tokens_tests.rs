//! That one grant is presented once, however many callers want a token at the same instant.
//!
//! Split from `allodia_tokens.rs` to keep that file under the 500-line limit; `#[path]` above
//! keeps it the same module, so `super::` still names the private items under test.

use std::sync::mpsc;

use mailcal_oauth::Secret;

use super::{Duration, REFRESH_SKEW, Tokens};
use crate::{
    LogLevel, MailcalApp,
    allodia::StoredAccount,
    tests::{ChannelObserver, NullLogger},
};

/// A demo app whose credential store **refuses** every write, which is what makes the
/// adoption tests below say something: the value has to end up correct in memory even when
/// persisting it cannot succeed.
fn app() -> std::sync::Arc<MailcalApp> {
    let (tx, _rx) = mpsc::channel();
    MailcalApp::new_demo(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        "Etc/UTC".to_owned(),
    )
}

/// A grant that was stored when the service lived on the old host.
fn signed_in_with(end_session: Option<&str>) -> StoredAccount {
    StoredAccount {
        // None, because what this fixture is about is a grant from an older build: the id is
        // exactly what such a grant does not carry.
        id: None,
        email: "person@example.test".to_owned(),
        name: None,
        refresh_token: "RT".to_owned(),
        granted_scopes: None,
        end_session_endpoint: end_session.map(str::to_owned),
    }
}

/// The regression this whole mechanism exists for.
///
/// Moving the account service to another host left every stored grant pointing at the old
/// host's sign-out endpoint, so signing out asked a service that no longer held the session
/// to end it. Discovery already runs once a launch to build the refresher, so the fresh answer
/// is free there; this is the part that spends it.
#[test]
fn a_moved_sign_out_endpoint_is_adopted() {
    let app = app();
    *app.allodia.lock().unwrap() =
        Some(signed_in_with(Some("https://old.example.test/end-session")));

    app.adopt_discovered_end_session(Some("https://new.example.test/end-session"));

    assert_eq!(
        app.allodia
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .end_session_endpoint
            .as_deref(),
        Some("https://new.example.test/end-session"),
    );
}

/// A service that stops advertising one is an answer, not a value to keep out of politeness:
/// holding the old URL would send somebody to end a session at a host that no longer offers
/// the endpoint.
#[test]
fn an_endpoint_that_goes_away_is_adopted_too() {
    let app = app();
    *app.allodia.lock().unwrap() =
        Some(signed_in_with(Some("https://old.example.test/end-session")));

    app.adopt_discovered_end_session(None);

    assert!(
        app.allodia
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .end_session_endpoint
            .is_none()
    );
}

/// Nobody signed in means no grant to keep current. Writing one here would invent an account,
/// and this runs on every token mint, so it has to be inert in that state rather than merely
/// harmless.
#[test]
fn adopting_invents_no_account_when_nobody_is_signed_in() {
    let app = app();
    assert!(app.allodia.lock().unwrap().is_none());

    app.adopt_discovered_end_session(Some("https://new.example.test/end-session"));

    assert!(app.allodia.lock().unwrap().is_none());
}

fn held(expires_in_minutes: i64) -> mailcal_oauth::TokenSet {
    mailcal_oauth::TokenSet {
        access_token: Secret::new("AT".to_owned()),
        refresh_token: None,
        expires_at: time::OffsetDateTime::now_utc() + Duration::minutes(expires_in_minutes),
        scope: String::new(),
        token_type: "Bearer".to_owned(),
    }
}

/// The regression this exists for, found by signing in again on a real account.
///
/// A token is held for about an hour, so a sign-in that stored a NEW grant and left the old
/// token cached went on presenting it, and the service refused it, because the new
/// authorisation superseded the grant it came from. On screen that is signing in successfully
/// and being told a fraction of a second later that you are signed out. Nothing cleared this
/// cache on sign-in OR on sign-out; it stayed hidden while the only way in was from a
/// signed-out state, where there is no stale token to present.
#[test]
fn forgetting_the_grant_forgets_the_token_minted_from_it() {
    let tokens = Tokens::default();
    let now = time::OffsetDateTime::now_utc();
    *tokens.access.lock().unwrap() = Some(held(45));
    assert_eq!(
        tokens.live(now).as_deref(),
        Some("AT"),
        "a live token is served from the cache, which is the whole reason a stale one is a bug"
    );
    tokens.forget();
    assert!(
        tokens.live(now).is_none(),
        "after the grant is replaced or erased, nothing minted from it may be presented again"
    );
}

/// ⚠️ The regression the refresh gate exists for, measured on a device.
///
/// Signing in drops the held access token, and the account list, the subscription card and the
/// purchase pass then ask for one at the same instant. Without the gate each read the same
/// stored refresh token and each presented it; the service rotates, so the second was a replay
/// and was answered `invalid_grant`. On screen that is a sign-in which succeeds and then says
/// the account is signed out, a fraction of a second before correcting itself:
///
/// ```text
/// 23:39:30.769  allodia: signed in; the grant is stored
/// 23:39:30.802  allodia: the access token has run out; refreshing the grant
/// 23:39:30.803  allodia: the access token has run out; refreshing the grant
/// 23:39:30.899  allodia: the account service REFUSED the stored sign-in
/// 23:39:30.900  allodia: the grant could not be refreshed; invalid_grant; invalid refresh token
/// 23:39:30.903  allodia: the sign-in is usable again
/// ```
///
/// A count rather than an absence of errors: the fake refresh here always succeeds, so a build
/// without the gate passes every assertion about the token and fails only on having minted it
/// eight times.
#[test]
fn many_callers_at_once_cost_one_refresh() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_time()
        .build()
        .expect("runtime");
    let tokens = std::sync::Arc::new(Tokens::default());
    let refreshes = std::sync::Arc::new(AtomicUsize::new(0));

    let served: Vec<String> = runtime.block_on(async {
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let tokens = std::sync::Arc::clone(&tokens);
            let refreshes = std::sync::Arc::clone(&refreshes);
            tasks.push(tokio::spawn(async move {
                tokens
                    .minted("a test", || async {
                        refreshes.fetch_add(1, Ordering::SeqCst);
                        // Long enough that every other caller is queued behind the gate, which
                        // is what a network round trip does on a device.
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        Ok(held(45))
                    })
                    .await
                    .expect("a token")
            }));
        }
        let mut served = Vec::new();
        for task in tasks {
            served.push(task.await.expect("a task"));
        }
        served
    });

    assert_eq!(
        refreshes.load(Ordering::SeqCst),
        1,
        "a caller that waited its turn must be served the winner's token, never refresh again \
         with the grant the winner has already spent"
    );
    assert_eq!(served.len(), 8);
    assert!(
        served.iter().all(|token| token == "AT"),
        "every caller is served the one token that was minted"
    );
}

#[test]
fn a_token_inside_the_skew_is_already_spent() {
    let tokens = Tokens::default();
    *tokens.access.lock().unwrap() = Some(held(REFRESH_SKEW.whole_minutes() - 1));
    assert!(
        tokens.live(time::OffsetDateTime::now_utc()).is_none(),
        "a token handed out with seconds left dies mid-request"
    );
}
