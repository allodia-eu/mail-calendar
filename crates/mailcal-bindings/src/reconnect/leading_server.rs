//! A JMAP server on loopback that sends the session request on to a plaintext address off this
//! device: a server that led there. One that later answers is a server that was fixed.

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

/// The plaintext address off this device the server leads to.
pub(super) const PLAINTEXT: &str = "http://mail.example.invalid";

/// The running server and its switches.
pub(super) struct Leading {
    /// Its address.
    pub(super) base: String,
    /// Whether it leads the session request to plaintext.
    pub(super) led: Arc<AtomicBool>,
    /// How many session requests it answers before it leads the next ones there, while `led`
    /// holds: a dial reads the session once for its mail and once more for each of its calendar
    /// and contacts, in that order.
    pub(super) answer_first: Arc<AtomicUsize>,
    /// How many session requests it has had: one for each connect a dial makes.
    pub(super) sessions: Arc<AtomicUsize>,
    /// Whether it refuses every credential, answering `401` to every request.
    pub(super) refuse: Arc<AtomicBool>,
}

/// Starts the server, leading every session request to plaintext. Its session offers mail, and
/// calendars and contacts beside it where `with_dav` asks for them.
pub(super) fn leading_server(with_dav: bool) -> Leading {
    let led = Arc::new(AtomicBool::new(true));
    let answer_first = Arc::new(AtomicUsize::new(0));
    let sessions = Arc::new(AtomicUsize::new(0));
    let refuse = Arc::new(AtomicBool::new(false));
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let base = format!(
        "http://{}",
        listener.local_addr().expect("the bound address")
    );
    let api = format!("{base}/jmap/");
    let (leading, budget, seen, refusing) = (
        Arc::clone(&led),
        Arc::clone(&answer_first),
        Arc::clone(&sessions),
        Arc::clone(&refuse),
    );
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 8192];
            let read = stream.read(&mut request).unwrap_or(0);
            // A session request; not one for the API's own resources, such as its push stream.
            let get =
                request[..read].starts_with(b"GET") && !request[..read].starts_with(b"GET /jmap/");
            if get {
                seen.fetch_add(1, Ordering::SeqCst);
            }
            let leads = get
                && leading.load(Ordering::SeqCst)
                && budget
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_err();
            let (status, headers, body) = if refusing.load(Ordering::SeqCst) {
                (
                    "401 Unauthorized",
                    "WWW-Authenticate: Basic realm=\"mail\"\r\n".to_owned(),
                    String::new(),
                )
            } else if leads {
                (
                    "307 Temporary Redirect",
                    format!("Location: {PLAINTEXT}/jmap/session\r\n"),
                    String::new(),
                )
            } else if get {
                ("200 OK", String::new(), session(&api, with_dav))
            } else {
                ("200 OK", String::new(), nothing_for(&request[..read]))
            };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    Leading {
        base,
        led,
        answer_first,
        sessions,
        refuse,
    }
}

/// An answer to every method call in `request` that finds nothing and changes nothing, so a
/// mailbox that connected syncs without error.
fn nothing_for(request: &[u8]) -> String {
    let body = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map_or(&[][..], |at| &request[at + 4..]);
    let calls = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|request| request.get("methodCalls").cloned())
        .and_then(|calls| calls.as_array().cloned())
        .unwrap_or_default();
    let responses: Vec<_> = calls
        .iter()
        .map(|call| {
            serde_json::json!([
                call[0],
                {
                    "accountId": "a", "state": "s", "oldState": "s", "newState": "s",
                    "queryState": "s", "list": [], "ids": [], "notFound": [],
                    "created": [], "updated": [], "destroyed": [], "hasMoreChanges": false,
                    "canCalculateChanges": false, "position": 0, "total": 0,
                },
                call[2],
            ])
        })
        .collect();
    serde_json::json!({ "methodResponses": responses, "sessionState": "s" }).to_string()
}

/// A session with mail on one account, and calendars and contacts where `with_dav` asks.
fn session(api: &str, with_dav: bool) -> String {
    let core = "urn:ietf:params:jmap:core";
    let mail = "urn:ietf:params:jmap:mail";
    let calendars = "urn:ietf:params:jmap:calendars";
    let contacts = "urn:ietf:params:jmap:contacts";
    let (capabilities, primary, account) = if with_dav {
        (
            serde_json::json!({ core: {}, mail: {}, calendars: {}, contacts: {} }),
            serde_json::json!({ mail: "a", calendars: "a", contacts: "a" }),
            serde_json::json!({ mail: {}, calendars: {}, contacts: {} }),
        )
    } else {
        (
            serde_json::json!({ core: {}, mail: {} }),
            serde_json::json!({ mail: "a" }),
            serde_json::json!({ mail: {} }),
        )
    };
    serde_json::json!({
        "capabilities": capabilities,
        "primaryAccounts": primary,
        "accounts": {
            "a": {
                "name": "fixed@example.org",
                "isPersonal": true,
                "isReadOnly": false,
                "accountCapabilities": account,
            },
        },
        "apiUrl": api,
        "downloadUrl": format!("{api}download/{{accountId}}/{{blobId}}/{{name}}?type={{type}}"),
        "uploadUrl": format!("{api}upload/{{accountId}}/"),
        "eventSourceUrl": format!("{api}eventsource/"),
        "state": "s",
    })
    .to_string()
}
