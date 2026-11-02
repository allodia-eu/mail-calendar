use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use super::{CallbackOutcome, OAuthLoopback, escaped, parse_request_target};
use crate::l10n;

#[test]
fn loopback_accepts_exactly_an_oauth_callback_and_answers_locally() {
    let loopback = OAuthLoopback::bind().expect("bind loopback");
    let address = loopback.address;
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let worker = std::thread::spawn(move || {
        loopback.wait_with_timeout(
            &worker_cancel,
            Duration::from_secs(2),
            None,
            "timeout",
            "failed",
        )
    });

    let mut browser = TcpStream::connect(address).expect("connect loopback");
    browser
        .write_all(b"GET /?code=fixture&state=expected HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .expect("write callback");
    let mut response = String::new();
    browser
        .read_to_string(&mut response)
        .expect("read response");

    let CallbackOutcome::Received(url) = worker.join().expect("worker") else {
        panic!("expected callback");
    };
    assert_eq!(
        url,
        format!("http://{address}/?code=fixture&state=expected")
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(&escaped(l10n::sign_in_page_done())));
}

#[test]
fn a_reauthentication_rebinds_the_exact_persisted_loopback_uri() {
    let original = OAuthLoopback::bind().expect("bind original loopback");
    let redirect = original.redirect_uri();
    drop(original);

    let rebound = OAuthLoopback::bind_redirect_uri(&redirect).expect("rebind persisted port");

    assert_eq!(rebound.redirect_uri(), redirect);
    assert!(OAuthLoopback::bind_redirect_uri("http://example.com:1234/").is_err());
    assert!(OAuthLoopback::bind_redirect_uri("http://127.0.0.1:1234/not-root").is_err());
}

#[test]
fn loopback_parser_rejects_non_callbacks() {
    assert!(parse_request_target(b"GET /favicon.ico HTTP/1.1\r\n\r\n").is_none());
    assert!(parse_request_target(b"POST /?code=x&state=y HTTP/1.1\r\n\r\n").is_none());
    assert_eq!(
        parse_request_target(b"GET /?error=access_denied&state=y HTTP/1.1\r\n\r\n"),
        Some("/?error=access_denied&state=y")
    );
}

#[test]
fn cancelling_stops_the_loopback_wait() {
    let loopback = OAuthLoopback::bind().expect("bind loopback");
    let cancel = AtomicBool::new(true);
    assert!(matches!(
        loopback.wait_with_timeout(&cancel, Duration::from_secs(2), None, "timeout", "failed",),
        CallbackOutcome::Cancelled
    ));
}

#[test]
fn state_filtered_wait_ignores_a_late_callback_from_a_cancelled_attempt() {
    let owner = OAuthLoopback::bind().expect("bind loopback");
    let address = owner.address;
    let loopback = owner.try_clone().expect("clone listener");
    assert_eq!(owner.redirect_uri(), loopback.redirect_uri());
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let worker = std::thread::spawn(move || {
        loopback.wait_with_timeout(
            &worker_cancel,
            Duration::from_secs(2),
            Some("current"),
            "timeout",
            "failed",
        )
    });

    let stale = request(address, "/?code=old&state=cancelled");
    assert!(stale.starts_with("HTTP/1.1 400 Bad Request"));
    let current = request(address, "/?code=new&state=current");
    assert!(current.starts_with("HTTP/1.1 200 OK"));

    let CallbackOutcome::Received(url) = worker.join().expect("worker") else {
        panic!("expected current callback");
    };
    assert_eq!(url, format!("http://{address}/?code=new&state=current"));
}

#[test]
fn a_browser_that_comes_back_late_reads_what_happened() {
    let loopback = OAuthLoopback::bind().expect("bind loopback");
    let address = loopback.address;
    assert!(matches!(
        loopback.wait_with_timeout(
            &AtomicBool::new(true),
            Duration::from_secs(2),
            None,
            "timeout",
            "failed"
        ),
        CallbackOutcome::Cancelled
    ));

    // Nothing is answered before the head is whole: closing with the rest of the request unread
    // resets the connection, and the browser loses the page.
    let mut browser = TcpStream::connect(address).expect("connect loopback");
    browser
        .write_all(b"GET /?code=late&state=cancelled HTTP/1.1\r\n")
        .expect("write line");
    browser
        .set_read_timeout(Some(Duration::from_millis(300)))
        .expect("read timeout");
    assert!(
        browser.read(&mut [0_u8; 1]).is_err(),
        "answered a partial head"
    );
    browser.set_read_timeout(None).expect("read timeout");
    browser
        .write_all(b"Host: 127.0.0.1\r\n\r\n")
        .expect("write headers");
    let mut late = String::new();
    browser.read_to_string(&mut late).expect("read response");

    assert!(late.starts_with("HTTP/1.1 200 OK"));
    assert!(late.contains(&escaped(l10n::sign_in_page_ended())));
}

#[test]
fn a_shared_port_goes_to_the_next_wait_rather_than_lingering() {
    let owner = OAuthLoopback::bind().expect("bind loopback");
    let address = owner.address;
    let first = owner.try_clone().expect("clone listener");
    assert!(!first.lingers);
    assert!(matches!(
        first.wait_with_timeout(
            &AtomicBool::new(true),
            Duration::from_secs(2),
            None,
            "timeout",
            "failed"
        ),
        CallbackOutcome::Cancelled
    ));

    let next = owner.try_clone().expect("clone listener");
    let worker = std::thread::spawn(move || {
        next.wait_with_timeout(
            &AtomicBool::new(false),
            Duration::from_secs(2),
            Some("current"),
            "timeout",
            "failed",
        )
    });
    // A lingering answer on the shared socket would take some of these.
    for _ in 0..5 {
        let stale = request(address, "/?code=old&state=cancelled");
        assert!(stale.starts_with("HTTP/1.1 400 Bad Request"));
    }
    assert!(request(address, "/?code=new&state=current").starts_with("HTTP/1.1 200 OK"));
    assert!(matches!(
        worker.join().expect("worker"),
        CallbackOutcome::Received(_)
    ));
}

#[test]
fn a_persisted_redirect_frees_its_port_when_the_wait_ends() {
    let original = OAuthLoopback::bind().expect("bind original loopback");
    let redirect = original.redirect_uri();
    drop(original);
    let rebound = OAuthLoopback::bind_redirect_uri(&redirect).expect("rebind persisted port");
    assert!(matches!(
        rebound.wait_with_timeout(
            &AtomicBool::new(true),
            Duration::from_secs(2),
            None,
            "timeout",
            "failed"
        ),
        CallbackOutcome::Cancelled
    ));

    assert!(OAuthLoopback::bind_redirect_uri(&redirect).is_ok());
}

fn request(address: std::net::SocketAddrV4, target: &str) -> String {
    let mut browser = TcpStream::connect(address).expect("connect loopback");
    write!(browser, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").expect("write callback");
    let mut response = String::new();
    browser
        .read_to_string(&mut response)
        .expect("read response");
    response
}
