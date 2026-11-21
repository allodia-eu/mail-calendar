//! What the listener says it is doing: listening once bound, and why not when it cannot be.

#![cfg(unix)]

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{ListenerStatus, McpConfig, McpServer, tests_fake::FakeBackend};

fn config(path: &str) -> McpConfig {
    McpConfig {
        endpoint: Some(path.to_owned()),
        accounts: BTreeSet::from(["work".to_owned()]),
        ..McpConfig::default()
    }
}

/// A socket path in a directory of the test's own, inside the 104-byte `sun_path` limit.
fn endpoint() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mcp.sock").to_string_lossy().into_owned();
    assert!(
        path.len() < 104,
        "the test path must itself fit in sun_path"
    );
    (dir, path)
}

/// Waits until `server` says something other than starting.
async fn settled(server: &McpServer) -> ListenerStatus {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let status = server.status();
            if status != ListenerStatus::Starting {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the listener settles")
}

fn server() -> McpServer {
    let (backend, _) = FakeBackend::new();
    McpServer::new(backend, tokio::runtime::Handle::current())
}

#[tokio::test(flavor = "multi_thread")]
async fn it_says_listening_only_once_bound_and_stopped_once_off() {
    let (_dir, path) = endpoint();
    let server = server();
    let changes = Arc::new(AtomicUsize::new(0));
    server.on_status_change({
        let changes = Arc::clone(&changes);
        move || {
            changes.fetch_add(1, Ordering::SeqCst);
        }
    });
    assert_eq!(server.status(), ListenerStatus::Stopped);

    server.apply(&config(&path));
    assert_eq!(settled(&server).await, ListenerStatus::Listening);
    assert!(
        tokio::net::UnixStream::connect(&path).await.is_ok(),
        "listening means a client can connect"
    );

    // A settings change that keeps the endpoint keeps the listener, and says nothing new.
    let heard = changes.load(Ordering::SeqCst);
    server.apply(&config(&path));
    assert_eq!(server.status(), ListenerStatus::Listening);
    assert_eq!(changes.load(Ordering::SeqCst), heard);

    server.apply(&McpConfig::default());
    assert_eq!(server.status(), ListenerStatus::Stopped);
    // Starting, listening, stopped: each heard once.
    assert_eq!(changes.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_copy_says_another_holds_the_endpoint() {
    let (_dir, path) = endpoint();
    let first = server();
    first.apply(&config(&path));
    assert_eq!(settled(&first).await, ListenerStatus::Listening);

    let second = server();
    let changes = Arc::new(AtomicUsize::new(0));
    second.on_status_change({
        let changes = Arc::clone(&changes);
        move || {
            changes.fetch_add(1, Ordering::SeqCst);
        }
    });
    second.apply(&config(&path));
    assert_eq!(settled(&second).await, ListenerStatus::AnotherInstance);
    assert_eq!(first.status(), ListenerStatus::Listening);
    assert_eq!(
        changes.load(Ordering::SeqCst),
        2,
        "the refusal is heard, so Settings stops saying it is starting"
    );

    second.stop();
    first.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_endpoint_that_cannot_be_used_says_unavailable() {
    let server = server();
    // Over the `sun_path` limit: refused before any syscall.
    server.apply(&config(&format!("/tmp/{}/mcp.sock", "x".repeat(120))));
    assert_eq!(server.status(), ListenerStatus::Unavailable);

    // A directory that is not one: refused by the listener itself.
    let file = tempfile::NamedTempFile::new().unwrap();
    let inside_a_file = file.path().join("mcp.sock");
    server.apply(&config(&inside_a_file.to_string_lossy()));
    assert_eq!(settled(&server).await, ListenerStatus::Unavailable);
}
