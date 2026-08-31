//! End-to-end over the **real** transport: the built `allodia-mcp` binary, a real socket, real
//! stdin and stdout.
//!
//! Everything else about this feature is testable in-process. This is not: framing, partial
//! reads, the not-running path, and above all the rule that **nothing but JSON-RPC reaches
//! stdout** are properties of the process, and a unit test of `relay()` would assert them
//! against a mock of the very thing that breaks. Needs no MCP client: a scripted session and a
//! socket are enough: so it runs in CI like any other test.

#![cfg(unix)]

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// A unique socket path for one test, well inside the 104-byte `sun_path` limit.
fn socket_path(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("a clock after 1970")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("amcp-{name}-{unique}.sock"));
    assert!(
        path.as_os_str().len() < 104,
        "the test socket path itself must fit in sun_path: {}",
        path.display(),
    );
    path
}

/// Spawns the relay against `endpoint`, with piped stdio.
fn spawn(endpoint: &std::path::Path) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_allodia-mcp"))
        .arg("--endpoint")
        .arg(endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the relay binary runs")
}

#[test]
fn a_request_reaches_the_socket_and_the_reply_reaches_stdout() {
    let path = socket_path("roundtrip");
    let listener = UnixListener::bind(&path).expect("the test listener binds");

    // A one-shot fake app: accept, read one line, answer it.
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the relay connects");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).expect("a framed request");
        let mut writer = stream;
        writeln!(
            writer,
            r#"{{"jsonrpc":"2.0","id":1,"result":{{"ok":true}}}}"#
        )
        .unwrap();
        writer.flush().unwrap();
        // Hold the connection open until the relay has had a chance to read.
        std::thread::sleep(Duration::from_millis(200));
        request
    });

    let mut child = spawn(&path);
    let mut stdin = child.stdin.take().expect("piped stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();

    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut response = String::new();
    stdout.read_line(&mut response).expect("a reply on stdout");

    let received = server.join().expect("the fake app finished");
    assert!(
        received.contains("\"method\":\"ping\""),
        "the request arrived at the socket verbatim: {received}",
    );
    assert_eq!(
        response.trim(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
        "and the reply arrived on stdout verbatim, one frame per line",
    );

    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_second_request_is_answered_too() {
    // Correct here for free (a socket's duplicated fd reads and writes at the same time) and
    // pinned anyway, because the Windows transport cannot (tests/relay_windows.rs) and this suite
    // is where a reader would look for the property. Every other test here sends exactly ONE
    // request, which is precisely the coverage gap that let the Windows relay ship deadlocking on
    // its second.
    let path = socket_path("second-request");
    let listener = UnixListener::bind(&path).expect("the test listener binds");

    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the relay connects");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = stream;
        let mut seen = 0;
        for id in [1, 2] {
            let mut request = String::new();
            if reader.read_line(&mut request).unwrap_or(0) == 0 {
                break;
            }
            seen += 1;
            writeln!(
                writer,
                r#"{{"jsonrpc":"2.0","id":{id},"result":{{"ok":true}}}}"#
            )
            .unwrap();
            writer.flush().unwrap();
        }
        std::thread::sleep(Duration::from_millis(100));
        seen
    });

    let mut child = spawn(&path);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

    // Read the first reply BEFORE sending the second request: that is what leaves the relay
    // waiting on the socket, which is the state the second write has to survive.
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize"}}"#).unwrap();
    stdin.flush().unwrap();
    let mut first = String::new();
    stdout.read_line(&mut first).expect("a reply to the first");
    assert_eq!(
        first.trim(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#
    );

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#).unwrap();
    stdin.flush().unwrap();
    let mut second = String::new();
    stdout
        .read_line(&mut second)
        .expect("a reply to the second");
    assert_eq!(
        second.trim(),
        r#"{"jsonrpc":"2.0","id":2,"result":{"ok":true}}"#
    );

    assert_eq!(server.join().expect("the fake app finished"), 2);
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn nothing_but_json_rpc_ever_reaches_stdout() {
    // The MCP stdio contract, and the failure it prevents: one stray line of diagnostics on
    // stdout desynchronizes the client's parser and the server looks broken rather than chatty.
    // Here the app is deliberately NOT running, which is exactly when a program is most tempted
    // to print an explanation.
    let path = socket_path("stdout-purity");
    let mut child = spawn(&path);
    let mut stdin = child.stdin.take().expect("piped stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":"abc","method":"tools/list"}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    drop(stdin);

    let output = child.wait_with_output().expect("the relay exits");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("stdout carried a non-JSON line ({err}): {line}"));
        assert_eq!(parsed["jsonrpc"], "2.0");
    }
    assert!(
        !String::from_utf8_lossy(&output.stderr).is_empty(),
        "the diagnostic went to stderr, where it belongs",
    );
}

#[test]
fn a_request_with_the_app_shut_down_gets_a_clean_error_carrying_its_own_id() {
    // The client must see a live-but-unavailable server, not a crashed one, and it must be able
    // to match the error to the call it made, or the request stays pending until it times out
    // and the symptom is a hang.
    let path = socket_path("not-running");
    let mut child = spawn(&path);
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
    assert!(
        first["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not running"),
        "and it says what to do about it",
    );

    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(second["id"], "second", "a string id round-trips too");
}

#[test]
fn a_reply_split_across_reads_still_arrives_as_one_frame() {
    // A socket can deliver a response in pieces; a relay that copied whatever a read returned
    // would emit half a JSON object and the client's line-based parser would choke on it.
    let path = socket_path("partial");
    let listener = UnixListener::bind(&path).expect("the test listener binds");

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the relay connects");
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        // Dribble the reply out in three writes, with the newline arriving last.
        for chunk in [
            r#"{"jsonrpc":"2.0","#,
            r#""id":1,"result":"#,
            "{\"ok\":true}}\n",
        ] {
            stream.write_all(chunk.as_bytes()).unwrap();
            stream.flush().unwrap();
            std::thread::sleep(Duration::from_millis(30));
        }
        std::thread::sleep(Duration::from_millis(100));
    });

    let mut child = spawn(&path);
    let mut stdin = child.stdin.take().expect("piped stdin");
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"ping"}}"#).unwrap();
    stdin.flush().unwrap();

    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut response = String::new();
    stdout.read_line(&mut response).expect("one whole frame");
    assert_eq!(
        response.trim(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
    );

    server.join().unwrap();
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_file(&path);
}
