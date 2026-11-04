use std::time::{Duration, SystemTime};

use super::{FLOW_LIFETIME, Poll, begin, poll};
use crate::{NextcloudError, Root, test_server};

/// The flow a server starts, with its URLs on `base`.
fn started(base: &str) -> String {
    format!(
        r#"{{"poll":{{"token":"poll-token-abc","endpoint":"{base}\/login\/v2\/poll"}},"login":"{base}\/login\/v2\/flow\/login-token"}}"#
    )
}

const GRANTED: &str =
    r#"{"server":"http:\/\/127.0.0.1","loginName":"f3a9c1","appPassword":"app-password-72"}"#;

#[tokio::test]
async fn a_flow_naming_another_server_is_refused() {
    let server = test_server::serve(vec![test_server::json(200, &started("http://127.0.0.1:1"))]);
    let root = Root::of(&server.base).unwrap();
    assert!(matches!(
        begin(&test_server::client(), &root, "client").await,
        Err(NextcloudError::ForeignUrl)
    ));
}

#[tokio::test]
async fn the_login_page_and_the_poll_come_from_the_flow() {
    let (server, root) = answering_with_own_urls(vec![]);
    let start = begin(
        &test_server::client(),
        &root,
        "Allodia Mail & Calendar (Linux)",
    )
    .await
    .unwrap();
    assert_eq!(
        start.login_url,
        format!("{}/login/v2/flow/login-token", root.as_str())
    );
    let request = server.requests().remove(0);
    assert!(
        request.starts_with("POST /index.php/login/v2 "),
        "{request}"
    );
    let agent = request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("user-agent:"))
        .unwrap();
    assert_eq!(agent, "user-agent: Allodia Mail & Calendar (Linux)");
    assert!(!format!("{:?}", start.pending).contains("poll-token-abc"));
}

#[tokio::test]
async fn a_poll_waits_until_the_grant_and_takes_it_once() {
    let (server, root) = answering_with_own_urls(vec![
        test_server::json(404, "[]"),
        test_server::json(500, ""),
        test_server::json(200, GRANTED),
    ]);
    let client = test_server::client();
    let start = begin(&client, &root, "client").await.unwrap();
    let now = SystemTime::now();
    assert_eq!(
        poll(&client, &start.pending, now).await.unwrap(),
        Poll::Waiting
    );
    assert_eq!(
        poll(&client, &start.pending, now).await.unwrap(),
        Poll::Waiting
    );
    let Poll::Granted(credentials) = poll(&client, &start.pending, now).await.unwrap() else {
        panic!("not granted");
    };
    assert_eq!(credentials.login_name, "f3a9c1");
    assert_eq!(credentials.app_password.expose(), "app-password-72");
    let request = server.requests().remove(1);
    assert!(request.starts_with("POST /login/v2/poll "), "{request}");
    assert!(request.ends_with("token=poll-token-abc"), "{request}");
}

#[tokio::test]
async fn a_server_that_cannot_be_reached_is_waited_for() {
    let (server, root) = answering_with_own_urls(vec![]);
    let client = test_server::client();
    let start = begin(&client, &root, "client").await.unwrap();
    drop(server);
    assert_eq!(
        poll(&client, &start.pending, SystemTime::now())
            .await
            .unwrap(),
        Poll::Waiting
    );
}

#[tokio::test]
async fn a_flow_past_its_lifetime_has_expired_without_asking() {
    let (server, root) = answering_with_own_urls(vec![]);
    let client = test_server::client();
    let start = begin(&client, &root, "client").await.unwrap();
    let later = SystemTime::now() + FLOW_LIFETIME + Duration::from_secs(1);
    assert_eq!(
        poll(&client, &start.pending, later).await.unwrap(),
        Poll::Expired
    );
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn an_unreadable_grant_is_an_error_because_it_comes_once() {
    let (_server, root) = answering_with_own_urls(vec![test_server::json(
        200,
        r#"{"server":"x","loginName":"","appPassword":"p"}"#,
    )]);
    let client = test_server::client();
    let start = begin(&client, &root, "client").await.unwrap();
    assert!(matches!(
        poll(&client, &start.pending, SystemTime::now()).await,
        Err(NextcloudError::Malformed)
    ));
}

#[tokio::test]
async fn a_server_that_starts_no_flow_is_refused() {
    for reply in [
        test_server::json(404, "[]"),
        test_server::json(200, "<html></html>"),
        test_server::json(200, r#"{"poll":{"token":"","endpoint":"/x"},"login":"/y"}"#),
    ] {
        let server = test_server::serve(vec![reply]);
        let root = Root::of(&server.base).unwrap();
        assert!(
            begin(&test_server::client(), &root, "client")
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn a_pending_flow_round_trips_with_every_field() {
    let (_server, root) = answering_with_own_urls(vec![]);
    let start = begin(&test_server::client(), &root, "client")
        .await
        .unwrap();
    let json = serde_json::to_string(&start.pending).unwrap();
    assert_eq!(
        serde_json::from_str::<super::PendingFlow>(&json).unwrap(),
        start.pending
    );
}

/// A server whose flow names URLs on itself, answering `then` after the start.
fn answering_with_own_urls(then: Vec<String>) -> (test_server::Server, Root) {
    let server = test_server::serve_with(|base| {
        let mut responses = vec![test_server::json(200, &started(base))];
        responses.extend(then);
        responses
    });
    let root = Root::of(&server.base).unwrap();
    (server, root)
}
