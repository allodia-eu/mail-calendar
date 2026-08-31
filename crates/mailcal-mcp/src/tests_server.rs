//! The server lifecycle over a **real** socket: start, serve, revoke, stop.
//!
//! The protocol tests run over an in-memory duplex, which proves the conversation but not the
//! listener. These prove the parts a user actually operates: that turning the setting on makes a
//! socket appear and answer, that unticking an account revokes access on a *running* server
//! rather than at the next restart, and that turning it off leaves nothing behind to connect to.

#![cfg(unix)]

use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

use crate::{McpConfig, McpServer, session::LEGACY_PROTOCOL_VERSIONS, tests_fake::FakeBackend};

/// A unique socket path for one test, well inside the 104-byte `sun_path` limit.
fn endpoint(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("mcp-{name}-{unique}/mcp.sock"));
    let path = path.to_string_lossy().into_owned();
    assert!(
        path.len() < 104,
        "the test path must itself fit in sun_path"
    );
    path
}

/// Connects and waits until the connection actually answers.
///
/// A plain `connect()` is not enough, and the reason is a real behaviour rather than a test
/// artefact: `apply` aborts the old listener and spawns a new one, and a client that connects in
/// the window between them lands in the dying listener's backlog and sees EOF. A real MCP client
/// reconnects, so does this. Anything that arrives ready answers on the first attempt.
async fn connect(path: &str) -> Option<UnixStream> {
    for _ in 0..100 {
        if let Ok(mut stream) = UnixStream::connect(path).await
            && let Some(reply) = try_ping(&mut stream).await
            && reply["result"] == json!({})
        {
            return Some(stream);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

/// A `ping` over `stream`, or `None` if the connection died before answering.
async fn try_ping(stream: &mut UnixStream) -> Option<Value> {
    let request = json!({"jsonrpc": "2.0", "id": 0, "method": "ping"});
    stream
        .write_all(format!("{request}\n").as_bytes())
        .await
        .ok()?;
    stream.flush().await.ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.ok()?;
    serde_json::from_str(&line).ok()
}

/// One request/response round trip over a connected socket.
async fn round_trip(stream: &mut UnixStream, request: &Value) -> Value {
    stream
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    stream.flush().await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).expect("one JSON line back")
}

/// The account ids `list_accounts` currently reports over `stream`.
async fn exposed_accounts(stream: &mut UnixStream) -> Vec<String> {
    let response = round_trip(
        stream,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "list_accounts"}}),
    )
    .await;
    response["result"]["structuredContent"]["accounts"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["account"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn config(path: &str, accounts: &[&str]) -> McpConfig {
    McpConfig {
        endpoint: Some(path.to_owned()),
        accounts: accounts.iter().map(|id| (*id).to_owned()).collect(),
        allow_direct_send: false,
        require_known_recipient: true,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_it_on_binds_a_socket_that_answers_and_turning_it_off_leaves_nothing() {
    let path = endpoint("lifecycle");
    let (backend, _) = FakeBackend::new();
    let server = McpServer::new(backend, tokio::runtime::Handle::current());

    // Off: no endpoint means no listener, whatever else the config says.
    server.apply(&McpConfig::default());
    assert!(!std::path::Path::new(&path).exists());

    server.apply(&config(&path, &["work"]));
    let mut stream = connect(&path).await.expect("the socket accepts");
    let init = round_trip(
        &mut stream,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": LEGACY_PROTOCOL_VERSIONS[0]},
        }),
    )
    .await;
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        "allodia-mail-and-calendar"
    );
    assert!(server.is_running());

    server.stop();
    assert!(!server.is_running());
    // A new client gets a refused connection rather than a hang: the negative the whole
    // "turn it off" affordance rests on.
    drop(stream);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        UnixStream::connect(&path).await.is_err(),
        "nothing answers on the endpoint once the server is stopped",
    );
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_settings_change_reaches_a_connection_that_is_already_open() {
    // THE regression. An MCP client opens one connection and holds it for the whole session, so
    // this test must NOT reconnect; reconnecting is precisely what a real client does not do, and
    // it is what made the previous version of this test pass over a broken build.
    //
    // What shipped: every connection captured an `Arc<McpConfig>` at accept time and kept it.
    // Ticking an account did nothing until the app was restarted (observed in the wild), and
    // unticking one did not revoke a live assistant's access: the same bug, in the direction that
    // actually matters. `apply` restarting the accept task could never fix it, because existing
    // connection tasks are untouched by that.
    let path = endpoint("live-config");
    let (backend, _) = FakeBackend::new();
    let server = McpServer::new(backend, tokio::runtime::Handle::current());

    // Start with NOTHING exposed, which is the shipped default.
    server.apply(&config(&path, &[]));
    let mut stream = connect(&path).await.expect("the socket accepts");
    assert!(
        exposed_accounts(&mut stream).await.is_empty(),
        "nothing is exposed to begin with",
    );

    // Tick an account. Same connection, no reconnect.
    server.apply(&config(&path, &["work"]));
    assert_eq!(
        exposed_accounts(&mut stream).await,
        ["work"],
        "the tick reached the connection the assistant already had open",
    );

    // And untick it again: the direction that is a revocation, not a convenience.
    server.apply(&config(&path, &[]));
    assert!(
        exposed_accounts(&mut stream).await.is_empty(),
        "the untick revoked access on the live connection, not at the next restart",
    );
    let refused = round_trip(
        &mut stream,
        &json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
                "params": {"name": "archive_message",
                           "arguments": {"account": "work", "key": "m1"}}}),
    )
    .await;
    assert_eq!(
        refused["result"]["isError"], true,
        "and acting on it is refused, not merely hidden from the listing",
    );

    server.stop();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_direct_send_on_reaches_a_live_connection_too() {
    // The same mechanism, on the toggle where a stale snapshot would be a security bug rather than
    // an inconvenience: a client that had `send_message` in a cached tool list must not be able to
    // call it after the user turns direct send off.
    let path = endpoint("live-send");
    let (backend, recorder) = FakeBackend::new();
    let server = McpServer::new(backend, tokio::runtime::Handle::current());

    server.apply(&McpConfig {
        allow_direct_send: true,
        ..config(&path, &["work"])
    });
    let mut stream = connect(&path).await.expect("the socket accepts");
    let send_request = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
                      "params": {"name": "send_message",
                                 "arguments": {"to": ["colleague@known.example"],
                                               "subject": "Hi", "body_text": "Hello"}}});
    let sent = round_trip(&mut stream, &send_request).await;
    assert_eq!(
        sent["result"]["isError"], false,
        "allowed while the toggle is on"
    );
    assert_eq!(recorder.lock().unwrap().sends.len(), 1);

    // Turn it off. The client still remembers the tool name.
    server.apply(&config(&path, &["work"]));
    let refused = round_trip(&mut stream, &send_request).await;
    assert!(
        refused.get("error").is_some() || refused["result"]["isError"] == true,
        "a remembered tool name cannot outlive the permission: {refused}",
    );
    assert_eq!(
        recorder.lock().unwrap().sends.len(),
        1,
        "and nothing further was sent",
    );

    server.stop();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_instance_does_not_steal_a_live_endpoint() {
    // THE trap. `unlink()` then `bind()` is the lazy fix and it is silently catastrophic: a
    // second copy of the app would delete the running instance's socket, bind its own, and every
    // MCP client would reconnect to the wrong process with the user seeing nothing.
    let path = endpoint("no-steal");
    let (backend_a, _) = FakeBackend::new();
    let first = McpServer::new(
        Arc::clone(&backend_a) as Arc<dyn crate::MailBackend>,
        tokio::runtime::Handle::current(),
    );
    first.apply(&config(&path, &["work"]));
    let mut held = connect(&path)
        .await
        .expect("the first instance is listening");

    let (backend_b, _) = FakeBackend::new();
    let second = McpServer::new(backend_b, tokio::runtime::Handle::current());
    second.apply(&config(&path, &["work"]));
    tokio::time::sleep(Duration::from_millis(200)).await;

    // The original connection still works, which it could not if the file had been replaced.
    let ping = round_trip(
        &mut held,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}),
    )
    .await;
    assert_eq!(ping["result"], json!({}));
    assert!(
        connect(&path).await.is_some(),
        "and the endpoint is still answering: the first instance still owns it",
    );

    second.stop();
    first.stop();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_socket_left_by_a_crash_is_replaced() {
    // The other half of "never steal": a leftover file that nobody is listening on must not
    // block the feature forever, or one crash would need a manual `rm` to recover from.
    let path = endpoint("stale");
    std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
    {
        // Bind and immediately drop, leaving the file with no listener behind it.
        let _dead = std::os::unix::net::UnixListener::bind(&path).unwrap();
    }
    assert!(std::path::Path::new(&path).exists());

    let (backend, _) = FakeBackend::new();
    let server = McpServer::new(backend, tokio::runtime::Handle::current());
    server.apply(&config(&path, &["work"]));
    assert!(
        connect(&path).await.is_some(),
        "the stale file was cleared and the endpoint bound",
    );

    server.stop();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_endpoint_that_cannot_be_bound_leaves_the_server_stopped_rather_than_retrying() {
    // A path over the sun_path limit is refused before any syscall, and the server stays off.
    // Retrying a bind that can never succeed is a log-spam generator, the user has to change
    // something either way.
    let (backend, _) = FakeBackend::new();
    let server = McpServer::new(backend, tokio::runtime::Handle::current());
    server.apply(&McpConfig {
        endpoint: Some(format!("/tmp/{}/mcp.sock", "x".repeat(120))),
        accounts: BTreeSet::from(["work".to_owned()]),
        ..McpConfig::default()
    });
    assert!(!server.is_running());
}
