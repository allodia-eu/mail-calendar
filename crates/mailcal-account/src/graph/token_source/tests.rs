//! Tests for [`super::GraphTokenSource`]: the refresh single-flight, the shared-failure
//! memo, and the reach classification that decides whether re-presenting a refresh token is
//! safe or is a replay. Its own file so the production type stays under the 500-line cap.

use std::{
    io::{Read, Write},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{
    test_support::{
        dead_grant_token_endpoint, flaky_token_endpoint, mock_token_endpoint,
        ratcheting_token_endpoint, refused_token_endpoint, source_at,
    },
    *,
};

/// The other half of the bug the single-flight was written for, and the half it missed.
///
/// Serializing the refresh made concurrent callers share a *success*. On a failure it
/// shared nothing: each waiter acquired the lock in turn, found no cached token, and
/// posted its own refresh presenting the **same** refresh token. One failed refresh on a
/// JMAP account, which runs mail, calendar and contacts providers over one token source;
/// therefore became one request per waiting provider.
///
/// Every request after the first is a replay of a token that may already be spent, and a
/// ratcheting server revokes the grant over exactly that. The previous test asserted
/// `hits == 1` only on the path where the server answers, so this hole was invisible.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_refresh_is_shared_with_queued_callers_and_never_reposted() {
    // Fails every request it sees, so any second request is a real second request.
    let (endpoint, hits) = flaky_token_endpoint(usize::MAX);
    let source = source_at(endpoint, None);

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let source = Arc::clone(&source);
        tasks.spawn(async move { source.access_token().await });
    }
    let results = tasks.join_all().await;

    assert!(
        results.iter().all(Result::is_err),
        "the endpoint failed every request, so no caller can hold a token: {results:?}",
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "each queued caller re-presented the same refresh token, that is the replay a \
         ratcheting server revokes the grant over",
    );
}

/// The classification that decides whether a retry is safe, driven end-to-end.
///
/// A connection nothing accepts fails before a byte of the request is written, so no
/// server saw the refresh token and it is still good: the backgrounded-Android case,
/// where the app's uid loses network access hundreds of times a day. It must carry the
/// short cool-down, or the account would park itself over failures that never left the
/// phone.
#[tokio::test]
async fn a_refusal_is_recorded_as_safe_to_retry_soon() {
    let source = source_at(refused_token_endpoint(), None);
    assert!(source.access_token().await.is_err());
    assert_eq!(
        source.last_failure(),
        Some((
            FailureKind::Unanswered(TokenRequestReach::NotSent),
            NOT_SENT_COOLDOWN.whole_seconds(),
        )),
    );
}

/// Its opposite: the request was delivered and then the conversation died. The server may
/// have refreshed and answered into a void, spending the token we presented: so this one
/// must back off far longer, and must not be reported as an expired sign-in, which would
/// put a re-authentication prompt in front of the user over a dropped packet.
#[tokio::test]
async fn a_delivered_request_that_dies_backs_off_without_prompting_a_reauth() {
    let (endpoint, _hits) = flaky_token_endpoint(usize::MAX);
    let source = source_at(endpoint, None);

    let err = source.access_token().await.unwrap_err();
    assert!(
        matches!(err, AccountError::Graph(_)),
        "a possibly-processed refresh is not proof of a dead grant: {err:?}",
    );
    assert_eq!(
        source.last_failure(),
        Some((
            FailureKind::Unanswered(TokenRequestReach::MaybeProcessed),
            MAYBE_PROCESSED_COOLDOWN.whole_seconds(),
        )),
    );
}

/// The cool-down releases, rather than wedging the account until it is restarted.
#[tokio::test]
async fn the_account_recovers_by_itself_once_the_cooldown_passes() {
    // One failure, then a healthy server.
    let (endpoint, hits) = flaky_token_endpoint(1);
    let source = source_at(endpoint, None);

    assert!(source.access_token().await.is_err());
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Still inside the cool-down: no second request, and no user-visible change.
    assert!(source.access_token().await.is_err());
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "retried inside the cool-down"
    );

    source.backdate_failure(MAYBE_PROCESSED_COOLDOWN + Duration::seconds(1));
    assert_eq!(
        source.access_token().await.unwrap(),
        "AT-OK",
        "the account never retried and stayed broken until a restart",
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert!(
        source.last_failure().is_none(),
        "a success must clear the remembered failure",
    );
}

/// The bug that killed a real Fastmail account, reduced to its mechanics.
///
/// Every provider on an OAuth account shares one token source: for JMAP that is mail,
/// calendar and contacts, and they sync concurrently. With no single-flight, each one that
/// finds the access token stale reads the *same* refresh token and posts its own refresh.
/// Against a server that merely rotates, that is wasteful. Against one that **ratchets**
/// (Fastmail: `invalid_grant; ratchet or client_id mismatch`) the replay revokes the whole
/// grant, and the account is dead at the next launch; after working all session on the
/// access token that was already cached.
///
/// So: concurrent callers must produce exactly **one** refresh request.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_callers_refresh_once_and_never_replay_a_rotated_token() {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let source = source_at(endpoint, None);

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let source = Arc::clone(&source);
        tasks.spawn(async move { source.access_token().await });
    }
    let results = tasks.join_all().await;

    for result in &results {
        assert!(
            result.is_ok(),
            "a concurrent caller replayed the refresh token and the server revoked the \
             grant: {result:?}",
        );
    }
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "one refresh must serve every waiting caller",
    );
}

/// The sibling property: once a rotation has happened, a later refresh must present the
/// **rotated** token, not the one it was built with. Without this the very next refresh is
/// itself a replay.
#[tokio::test]
async fn a_later_refresh_presents_the_rotated_token() {
    let (endpoint, hits) = ratcheting_token_endpoint("initial-refresh");
    let source = source_at(endpoint, None);

    assert_eq!(source.access_token().await.unwrap(), "AT-1");
    // Expire the cache so the next call must go back to the server.
    source.seed_access_token(String::new(), OffsetDateTime::UNIX_EPOCH);
    assert_eq!(
        source.access_token().await.unwrap(),
        "AT-2",
        "the second refresh presented a stale token and was rejected as a replay",
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn access_token_refreshes_once_then_serves_from_cache() {
    let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT"}"#;
    let (endpoint, hits) = mock_token_endpoint(vec![body.to_owned()]);
    let source = source_at(endpoint, None);

    assert_eq!(source.access_token().await.unwrap(), "AT");
    // A fresh token (1h) is cached: the second call must not hit the endpoint again.
    assert_eq!(source.access_token().await.unwrap(), "AT");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_rotated_refresh_token_is_reported_to_the_sink() {
    struct Recorder(Mutex<Vec<String>>);
    #[async_trait]
    impl TokenSink for Recorder {
        async fn refresh_token_rotated(&self, _account: &AccountId, new_refresh_token: &str) {
            self.0.lock().unwrap().push(new_refresh_token.to_owned());
        }
    }
    let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
    let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT","refresh_token":"ROTATED"}"#;
    let (endpoint, _hits) = mock_token_endpoint(vec![body.to_owned()]);
    let source = source_at(endpoint, Some(Arc::clone(&recorder) as Arc<dyn TokenSink>));

    source.access_token().await.unwrap();
    assert_eq!(recorder.0.lock().unwrap().as_slice(), ["ROTATED"]);
}

#[tokio::test]
async fn an_invalid_grant_refresh_is_a_reauth_signal() {
    let body = r#"{"error":"invalid_grant","error_description":"revoked"}"#;
    // A 200 wrapper won't do: the endpoint must be non-2xx for an error body.
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            hits_thread.fetch_add(1, Ordering::SeqCst);
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    let source = source_at(format!("http://{addr}/token"), None);
    let err = source.access_token().await.unwrap_err();
    assert!(matches!(err, AccountError::SigninRejected(_)));
}

/// A grant that predates a scope this build wants is refused the same way, and must reach the
/// same prompt.
///
/// It used to fall into the transient arm, which is the worst of both: the refresh cannot ever
/// succeed, so the account retried on every cool-down for as long as it existed, and the person
/// was never told the one thing that would fix it. A refresh no longer names a scope
/// (`mailcal-oauth`), so this should be unreachable from our own requests: a server may still
/// raise it, and it is the classification rather than the request that decides what a caller does.
#[tokio::test]
async fn an_invalid_scope_refresh_is_a_reauth_signal_too_not_something_to_retry() {
    let body = r#"{"error":"invalid_scope","error_description":"unable to issue scope Mail.Send"}"#;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    let source = source_at(format!("http://{addr}/token"), None);
    let err = source.access_token().await.unwrap_err();
    assert!(
        matches!(err, AccountError::SigninRejected(_)),
        "an under-scoped grant needs consent, not a retry; got {err:?}"
    );
}

/// A grant the server has refused is not asked about again, and every caller behind the memo
/// still learns the sign-in is dead.
///
/// Both halves matter, and the second is why the memo carries a *kind* rather than only a reach.
/// `invalid_grant` deliberately bypassed the cool-down at first, so a production log shows four
/// refreshes in under two minutes (08:39:56 → 08:41:41), each presenting a token the server had
/// already rejected, and on an account with several providers, each of them queues up to do it
/// again. But a memo that flattened this into a transient error would be worse than the traffic:
/// the "sign in again" prompt would then appear only for whichever caller happened to reach the
/// server first, and every other one would report an outage over a credential that is dead.
#[tokio::test]
async fn a_dead_grant_is_not_asked_about_again_and_still_reads_as_an_expired_signin() {
    let (endpoint, hits) = dead_grant_token_endpoint();
    let source = source_at(endpoint, None);

    let first = source.access_token().await.unwrap_err();
    assert!(
        matches!(first, AccountError::SigninRejected(_)),
        "{first:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    for _ in 0..3 {
        let err = source.access_token().await.unwrap_err();
        assert!(
            matches!(err, AccountError::SigninRejected(_)),
            "a remembered dead grant must keep raising the re-authentication prompt rather than \
             degrading to an outage: {err:?}",
        );
    }

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the account re-presented a refresh token the server had already refused",
    );
    assert_eq!(
        source.last_failure(),
        Some((FailureKind::DeadGrant, DEAD_GRANT_COOLDOWN.whole_seconds())),
    );
}

/// Signing back in clears the memo, which is the only reason a 30-minute cool-down on a dead
/// grant is safe to hold.
///
/// The claim being checked is a claim about *recovery*: the two paths that fix a dead grant either
/// build a whole new token source (JMAP re-auth replaces the registry entry) or seed this one with
/// the freshly exchanged access token. If the second did not clear the failure, a user who had
/// just re-authenticated would sit and watch a stale mailbox for half an hour with nothing to do
/// about it: a far worse bug than the one the memo fixes.
#[tokio::test]
async fn seeding_a_fresh_sign_in_clears_a_remembered_dead_grant() {
    let (endpoint, hits) = dead_grant_token_endpoint();
    let source = source_at(endpoint, None);
    assert!(source.access_token().await.is_err());
    assert!(source.last_failure().is_some());

    // What every OAuth completion path does with the access token the code exchange returned.
    source.seed_access_token(
        "AT-FRESH".to_owned(),
        OffsetDateTime::now_utc() + Duration::hours(1),
    );

    assert_eq!(
        source.access_token().await.unwrap(),
        "AT-FRESH",
        "a re-authenticated account is still parked behind the cool-down of the credential it \
         replaced",
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the seeded token is used as-is, nothing needs refreshing yet",
    );
    assert!(source.last_failure().is_none());
}
