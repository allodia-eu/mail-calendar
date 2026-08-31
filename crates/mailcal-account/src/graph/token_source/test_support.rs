//! Mock token endpoints for [`super::GraphTokenSource`]'s tests and the sibling provider tests.
//!
//! The one that matters is [`ratcheting_token_endpoint`]: it models an authorization server that
//! rotates the refresh token on every refresh **and treats a replay as theft**, revoking the
//! grant. That is Fastmail's behaviour, and it is the only kind of server on which a concurrent
//! double-refresh is fatal rather than merely wasteful: so it is the only kind that can catch
//! the regression this module exists to guard.

use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use engine_core::ids::AccountId;
use mailcal_oauth::{OAuthClient, OAuthProviderConfig};

use super::{GraphTokenSource, TokenSink};

/// A mock token endpoint that models a **rotating, ratcheting** authorization server;
/// Fastmail's behaviour, and the one that turns a harmless-looking race into a dead
/// account.
///
/// It holds exactly one currently-valid refresh token. A refresh presenting it rotates to
/// the next one and succeeds; a refresh presenting **any** other value; including the one
/// that was valid a moment ago, is a replay, and the server answers `invalid_grant` *and
/// revokes the grant for good*. That last part is what makes a concurrent double-refresh
/// unrecoverable rather than merely wasteful: the second request does not just fail, it
/// takes the account with it.
///
/// Returns the endpoint URL and the count of refresh requests it saw.
pub(crate) fn ratcheting_token_endpoint(initial: &str) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut valid = initial.to_owned();
    let mut generation = 0usize;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let request = read_request(&mut stream);
            hits_thread.fetch_add(1, Ordering::SeqCst);
            let presented = form_value(&request, "refresh_token").unwrap_or_default();
            // The ratchet: anything but the live token revokes the grant outright, so
            // every later refresh fails too; exactly like the real server.
            let body = if presented == valid && !valid.is_empty() {
                generation += 1;
                valid = format!("rotated-{generation}");
                format!(
                    r#"{{"token_type":"Bearer","expires_in":3600,"access_token":"AT-{generation}","refresh_token":"{valid}"}}"#
                )
            } else {
                valid.clear();
                r#"{"error":"invalid_grant","error_description":"ratchet or client_id mismatch"}"#
                    .to_owned()
            };
            let status = if body.contains("\"error\"") {
                "400 Bad Request"
            } else {
                "200 OK"
            };
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}/token"), hits)
}

/// A token endpoint that refuses every refresh with `invalid_grant`, and counts how many it was
/// asked. Unlike the one-shot listener the older `an_invalid_grant_refresh_is_a_reauth_signal`
/// builds inline, this keeps answering, which is the only way to tell "we stopped asking" from
/// "there was nobody left to ask".
pub(crate) fn dead_grant_token_endpoint() -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = read_request(&mut stream);
            hits_thread.fetch_add(1, Ordering::SeqCst);
            let body = r#"{"error":"invalid_grant","error_description":"revoked"}"#;
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}/token"), hits)
}

/// Reads a whole small HTTP request, honouring `Content-Length`: a single `read` can stop
/// on the headers and leave the form body behind, which would make the mock above decide a
/// legitimate refresh was a replay and turn this into a flaky test.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(read) = stream.read(&mut buf) {
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..read]);
        let text = String::from_utf8_lossy(&raw);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let length: usize = head
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .or_else(|| line.strip_prefix("content-length: "))
            })
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        if body.len() >= length {
            break;
        }
    }
    String::from_utf8_lossy(&raw).into_owned()
}

/// One `application/x-www-form-urlencoded` value out of a request body.
fn form_value(request: &str, key: &str) -> Option<String> {
    let (_, body) = request.split_once("\r\n\r\n")?;
    body.split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}

/// A mock token endpoint that **accepts** the request and then hangs up without answering,
/// for the first `failures` requests, and serves a valid token response after that.
///
/// The hang-up is the point. A refused connection fails before a byte is written and is
/// provably safe to retry; this one fails *after* the request was delivered, which is
/// indistinguishable from a server that processed the refresh and lost the answer. It is the
/// shape that must never be retried in a tight loop, and, because the request really does
/// arrive; it is the only shape whose retries a test can count.
///
/// Returns the endpoint URL and the count of requests it saw.
pub(crate) fn flaky_token_endpoint(failures: usize) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for (i, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { continue };
            let _ = read_request(&mut stream);
            hits_thread.fetch_add(1, Ordering::SeqCst);
            if i < failures {
                // Drop it mid-conversation: the request landed, no response comes back.
                continue;
            }
            let body = r#"{"token_type":"Bearer","expires_in":3600,"access_token":"AT-OK"}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}/token"), hits)
}

/// A token endpoint URL that nothing is listening on, so a connection to it is refused
/// before any byte of the request is written.
pub(crate) fn refused_token_endpoint() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}/token")
}

/// A mock token endpoint that serves each queued response once, counting hits: so a
/// test can prove the second `access_token()` was served from cache (no second hit).
pub(crate) fn mock_token_endpoint(responses: Vec<String>) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_thread = Arc::clone(&hits);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for (i, stream) in listener.incoming().enumerate() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            hits_thread.fetch_add(1, Ordering::SeqCst);
            let body = responses.get(i).cloned().unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}/token"), hits)
}

/// A **distinct** account for every test source.
///
/// Not cosmetic. Token state is now shared per account across a process, so two sources built with
/// the same id share one state, which is the property under test in one place and a hidden
/// coupling between unrelated tests everywhere else. They all used
/// `alice@example.com@graph.microsoft.com`, so a failure memo left by one would have leaked into
/// the next: green or red depending on which order the harness happened to run them in.
fn unique_account() -> AccountId {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    AccountId::try_from(format!("alice+{n}@example.com@graph.microsoft.com").as_str())
        .expect("a valid account id")
}

pub(crate) fn source_at(
    token_endpoint: String,
    sink: Option<Arc<dyn TokenSink>>,
) -> Arc<GraphTokenSource> {
    source_for(unique_account(), token_endpoint, sink)
}

/// A source over a **named** account, for the one test that needs two sources to be the same
/// account; i.e. two cores in one process, which is what
/// [`CredentialOrigin`](crate::CredentialOrigin) exists for.
pub(crate) fn source_for(
    account: AccountId,
    token_endpoint: String,
    sink: Option<Arc<dyn TokenSink>>,
) -> Arc<GraphTokenSource> {
    let provider = OAuthProviderConfig {
        authorize_endpoint: "https://example/authorize".to_owned(),
        token_endpoint,
        client_id: "client-abc".to_owned(),
        client_secret: None,
        redirect_uri: "eu.allodia.mailcal://oauth".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        resource: None,
        style: mailcal_oauth::AuthStyle::Microsoft,
    };
    GraphTokenSource::from_parts(
        OAuthClient::new(provider).unwrap(),
        account,
        "initial-refresh".to_owned(),
        sink,
        "graph",
        crate::CredentialOrigin::Stored,
    )
}
