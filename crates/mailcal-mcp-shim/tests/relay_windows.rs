//! End-to-end over the **real** Windows transport: the built `allodia-mcp` binary, a real named
//! pipe, real stdin and stdout.
//!
//! This file exists because its absence shipped a relay that deadlocked on every client. The Unix
//! suite next door is `#![cfg(unix)]` and each of its tests sends exactly **one** request, which
//! is the one request the broken Windows build could answer. `initialize` came back; `tools/list`
//! hung forever; the symptom read as a broken *server*.
//!
//! So both tests here are about the **second** message. A pipe opened as a file is a synchronous
//! file object and Windows serializes I/O on one, so the reader parked on a reply blocked the
//! writer from sending anything more (`src/windows.rs`). Either shape below reproduces it, and
//! they are separate cases rather than one longer script because they fail for reasons a reader
//! should not have to disentangle: the first is an ordinary conversation, the second is a
//! notification, which draws no reply at all, so the reader is parked with nothing coming.

#![cfg(windows)]

use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncBufReader},
    net::windows::named_pipe::ServerOptions,
};

/// A unique pipe name for one test, so a leftover server from another test cannot answer.
fn pipe_name(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_nanos();
    format!(r"\\.\pipe\amcp-test-{name}-{unique}")
}

/// Spawns the relay against `endpoint`, with piped stdio.
fn spawn(endpoint: &str) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_allodia-mcp"))
        .arg("--endpoint")
        .arg(endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the relay binary runs")
}

/// A fake app on `endpoint`: answers any line carrying an `"id"` with a result echoing it, and
/// stays silent for anything else: the same one-frame-per-request, nothing-for-a-notification
/// contract `mailcal-mcp`'s session loop keeps.
fn serve_fake_app(endpoint: String) -> std::thread::JoinHandle<Vec<String>> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a test runtime");
        runtime.block_on(async move {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&endpoint)
                .expect("the test pipe binds");
            server.connect().await.expect("the relay connects");
            let (reader, mut writer) = tokio::io::split(server);
            let mut lines = AsyncBufReader::new(reader).lines();
            let mut received = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                received.push(line.clone());
                // Mirrors the real server: a notification (no id) is never answered.
                if let Some(id) = id_of(&line) {
                    let frame = format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{{"ok":true}}}}"#);
                    if writer
                        .write_all(format!("{frame}\n").as_bytes())
                        .await
                        .is_err()
                    {
                        break;
                    }
                    let _ = writer.flush().await;
                }
            }
            received
        })
    })
}

/// The raw JSON text of a line's `id`, or `None` for a notification.
fn id_of(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    match value.get("id")? {
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// Long enough for the relay to have settled into waiting on the pipe with nothing to read.
///
/// **This is what makes the test reproduce the bug**, and it is not padding. The deadlock needs a
/// read to be *outstanding* when the next write is attempted; two lines written back-to-back beat
/// the reader to the handle and sail through, so a script that simply pipes both in at once passes
/// against the broken relay. Written without this settle, both tests below did exactly that.
const SETTLE: Duration = Duration::from_millis(250);

/// The relay's stdout, read on a background thread so a wedged relay fails the test instead of
/// hanging it.
struct Frames(std::sync::mpsc::Receiver<String>);

impl Frames {
    fn from(stdout: std::process::ChildStdout) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if tx.send(line.trim().to_owned()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self(rx)
    }

    /// The next frame, or a failed test naming the deadlock rather than a bare timeout.
    fn next(&self, what: &str) -> String {
        self.0
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|_| {
                panic!(
                    "no reply to {what}: the relay is wedged, which is exactly the deadlock this \
                 test exists for",
                )
            })
    }
}

#[test]
fn a_second_request_is_answered_too() {
    // THE regression. One request always worked; the relay was then waiting on the pipe for the
    // app to speak, and on Windows that wait blocked its own next write: so the client's very
    // next call hung. Reading the first reply before sending the second request is what puts the
    // relay in that state deterministically.
    let endpoint = pipe_name("second-request");
    let app = serve_fake_app(endpoint.clone());
    // Give the listener a moment to bind before the relay dials it.
    std::thread::sleep(SETTLE);

    let mut child = spawn(&endpoint);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let frames = Frames::from(child.stdout.take().expect("piped stdout"));

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize"}}"#).unwrap();
    stdin.flush().unwrap();
    assert_eq!(
        frames.next("the first request"),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
    );
    std::thread::sleep(SETTLE);

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    stdin.flush().unwrap();
    assert_eq!(
        frames.next("the second request"),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#,
    );

    drop(stdin);
    let _ = child.wait();
    let received = app.join().expect("the fake app finished");
    assert_eq!(received.len(), 2, "and both requests reached the pipe");
}

#[test]
fn a_notification_does_not_wedge_the_request_after_it() {
    // The same deadlock, in the shape MCP actually opens with: `notifications/initialized` is sent
    // right after the handshake and draws no reply, so the relay is left waiting on a pipe that
    // will stay silent until someone writes, which, while it waited, it could not do.
    let endpoint = pipe_name("notification");
    let app = serve_fake_app(endpoint.clone());
    std::thread::sleep(SETTLE);

    let mut child = spawn(&endpoint);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let frames = Frames::from(child.stdout.take().expect("piped stdout"));

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    // Nothing comes back from a notification, so there is no reply to wait on: this is the only
    // place a sleep is doing the synchronising, and it is the whole point of the case.
    std::thread::sleep(SETTLE);

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":9,"method":"ping"}}"#).unwrap();
    stdin.flush().unwrap();
    assert_eq!(
        frames.next("the request after the notification"),
        r#"{"jsonrpc":"2.0","id":9,"result":{"ok":true}}"#,
        "the notification itself drew no reply, and did not wedge what followed",
    );

    drop(stdin);
    let _ = child.wait();
    let received = app.join().expect("the fake app finished");
    assert_eq!(received.len(), 2, "both lines reached the pipe verbatim");
}

#[test]
fn a_request_with_the_app_shut_down_gets_a_clean_error_carrying_its_own_id() {
    // The Windows half of the Unix suite's not-running case: the client must see a live-but-
    // unavailable server, not a crashed one, and must be able to match the error to its call.
    // Nothing is listening on this name.
    let endpoint = pipe_name("not-running");
    let mut child = spawn(&endpoint);
    let mut stdin = child.stdin.take().expect("piped stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":41,"method":"tools/list"}}"#
    )
    .unwrap();
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":"second","method":"ping"}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let output = child.wait_with_output().expect("the relay exits cleanly");
    assert!(
        output.status.success(),
        "the relay exits 0; it was unavailable, not broken",
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 2, "both requests were answered: {stdout}");

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first["id"], 41);
    assert_eq!(first["error"]["code"], -32000);

    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second["id"], "second", "a string id round-trips too");

    // And the MCP stdio contract holds where a program is most tempted to break it.
    assert!(
        !String::from_utf8_lossy(&output.stderr).is_empty(),
        "the diagnostic went to stderr, where it belongs",
    );
}
