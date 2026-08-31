//! Fetcher tests against one-shot `127.0.0.1` TCP servers (the mock pattern from
//! `mailcal-oauth`), so the real reqwest path runs fully offline. Every mock is plain
//! HTTP, which also pins the safety property that an HTTP hop is never trusted.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use url::Url;

use super::{FetchOutcome, Fetcher};
use crate::DetectConfig;

/// A canned HTTP response: status line, extra headers, and a body with a correct
/// `Content-Length`.
fn response(status_line: &str, extra_headers: &[(&str, &str)], body: &str) -> String {
    let mut head = format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in extra_headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    format!("{head}\r\n{body}")
}

/// A `301` redirect to `location`.
fn redirect_to(location: &str) -> String {
    response("301 Moved Permanently", &[("Location", location)], "")
}

/// Binds a `127.0.0.1` server that serves `responses` in order, one per accepted
/// connection, then stops. Returns its base `http://addr` URL.
fn mock_server(responses: Vec<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for body in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(body.as_bytes());
            let _ = stream.flush();
        }
    });
    base
}

/// A server that accepts one connection and never replies, to exercise the timeout.
fn silent_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(30));
            drop(stream);
        }
    });
    base
}

fn fetcher(config: &DetectConfig) -> Fetcher {
    Fetcher::new(config).unwrap()
}

fn url(base: &str, path: &str) -> Url {
    Url::parse(&format!("{base}{path}")).unwrap()
}

#[tokio::test]
async fn success_returns_body_and_is_untrusted_over_http() {
    let base = mock_server(vec![response("200 OK", &[], "<clientConfig/>")]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/config"))
        .await;
    let FetchOutcome::Response(resp) = outcome else {
        panic!("expected a response, got {outcome:?}");
    };
    assert!(resp.is_success());
    assert_eq!(resp.body, b"<clientConfig/>");
    assert!(!resp.trusted, "an http hop must never be trusted");
    assert!(!resp.www_authenticate);
}

#[tokio::test]
async fn not_found_is_a_response_with_a_non_success_status() {
    let base = mock_server(vec![response("404 Not Found", &[], "nope")]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/config"))
        .await;
    let FetchOutcome::Response(resp) = outcome else {
        panic!("expected a response, got {outcome:?}");
    };
    assert_eq!(resp.status, 404);
    assert!(!resp.is_success());
}

#[tokio::test]
async fn refused_connection_is_a_network_error() {
    // Bind then drop to obtain a port nothing is listening on.
    let addr = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let outcome = fetcher(&DetectConfig::default())
        .get(&Url::parse(&format!("http://{addr}/config")).unwrap())
        .await;
    assert!(
        matches!(outcome, FetchOutcome::NetworkError),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn a_redirect_chain_is_followed_to_the_final_response() {
    let base = mock_server(vec![
        redirect_to("/second"),
        redirect_to("/third"),
        response("200 OK", &[], "final"),
    ]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/first"))
        .await;
    let FetchOutcome::Response(resp) = outcome else {
        panic!("expected a response, got {outcome:?}");
    };
    assert_eq!(resp.body, b"final");
    assert_eq!(resp.final_url.path(), "/third");
}

#[tokio::test]
async fn too_many_redirects_is_a_miss() {
    let base = mock_server(vec![redirect_to("/loop"); 8]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/loop"))
        .await;
    assert!(matches!(outcome, FetchOutcome::Miss), "got {outcome:?}");
}

#[tokio::test]
async fn a_redirect_without_location_is_a_miss() {
    let base = mock_server(vec![response("302 Found", &[], "")]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/config"))
        .await;
    assert!(matches!(outcome, FetchOutcome::Miss), "got {outcome:?}");
}

#[tokio::test]
async fn a_body_over_the_cap_is_a_miss() {
    let base = mock_server(vec![response("200 OK", &[], &"x".repeat(1000))]);
    let config = DetectConfig {
        max_body_bytes: 64,
        ..DetectConfig::default()
    };
    let outcome = fetcher(&config).get(&url(&base, "/config")).await;
    assert!(matches!(outcome, FetchOutcome::Miss), "got {outcome:?}");
}

#[tokio::test]
async fn www_authenticate_is_detected_on_a_401() {
    let base = mock_server(vec![response(
        "401 Unauthorized",
        &[("WWW-Authenticate", "Basic realm=\"jmap\"")],
        "",
    )]);
    let outcome = fetcher(&DetectConfig::default())
        .get(&url(&base, "/.well-known/jmap"))
        .await;
    let FetchOutcome::Response(resp) = outcome else {
        panic!("expected a response, got {outcome:?}");
    };
    assert_eq!(resp.status, 401);
    assert!(resp.www_authenticate);
    assert!(!resp.is_success());
}

#[tokio::test]
async fn a_slow_server_hits_the_request_timeout() {
    let base = silent_server();
    let config = DetectConfig {
        http_timeout: Duration::from_millis(250),
        ..DetectConfig::default()
    };
    let outcome = fetcher(&config).get(&url(&base, "/config")).await;
    assert!(
        matches!(outcome, FetchOutcome::NetworkError),
        "got {outcome:?}"
    );
}
