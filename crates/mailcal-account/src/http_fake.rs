//! The one loopback HTTP server the crate's tests talk to: the Graph and Google wrappers', the
//! JMAP connect tests' (`crate::jmap`) and the token endpoints of `GraphTokenSource`'s tests. It
//! reads each request whole, its body included, before it answers, one request a connection,
//! and records what every request carried.
//!
//! Reading the whole request first is what makes a hang-up mean one thing: the client has
//! finished sending, so a request the server then drops is one that may have been received.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// How the server answers one request.
#[derive(Debug, Clone)]
pub(crate) enum Reply {
    /// A `200` with this JSON body.
    Json(String),
    /// This status line (`400 Bad Request`) with this JSON body.
    Status(&'static str, String),
    /// A `401` whose challenge offers `Bearer`: the token was refused.
    RefusedBearer,
    /// The connection is closed with no answer, as a socket the OS killed is.
    HangUp,
}

/// One request the server received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seen {
    /// `GET`, `POST` and so on.
    pub(crate) method: String,
    /// The request target, with its query.
    pub(crate) path: String,
    /// The whole `Authorization` header value, empty when none was sent.
    pub(crate) authorization: String,
    /// The request body, decoded from its chunks when it came in them.
    pub(crate) body: String,
}

/// What the server has received so far.
pub(crate) type Received = Arc<Mutex<Vec<Seen>>>;

/// Starts a server on loopback answering `replies` in order, and `404` once they run out.
/// Returns its base URL, `http://127.0.0.1:<port>`, and what it received.
pub(crate) fn serve(replies: Vec<Reply>) -> (String, Received) {
    let mut replies = replies.into_iter();
    serve_with(move |_| {
        replies
            .next()
            .unwrap_or_else(|| Reply::Status("404 Not Found", String::new()))
    })
}

/// Starts a server on loopback answering each request as `answer` decides, from what it carried.
/// Returns its base URL and what it received.
pub(crate) fn serve_with(
    mut answer: impl FnMut(&Seen) -> Reply + Send + 'static,
) -> (String, Received) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("address"));
    let received: Received = Arc::default();
    let recorder = Arc::clone(&received);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Some(request) = read_request(&stream) else {
                continue;
            };
            let reply = answer(&request);
            recorder
                .lock()
                .expect("received mutex poisoned")
                .push(request);
            let response = match reply {
                Reply::Json(body) => response("200 OK", "", &body),
                Reply::Status(status, body) => response(status, "", &body),
                Reply::RefusedBearer => response(
                    "401 Unauthorized",
                    "WWW-Authenticate: Bearer error=\"invalid_token\"\r\n",
                    "",
                ),
                Reply::HangUp => continue,
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (base, received)
}

/// A token endpoint's answer minting `AT-<n>`, good for an hour.
pub(crate) fn minted(n: u8) -> String {
    format!(r#"{{"token_type":"Bearer","expires_in":3600,"access_token":"AT-{n}"}}"#)
}

/// What a request carrying the token `AT-<n>` presents.
pub(crate) fn bearer(n: u8) -> String {
    format!("Bearer AT-{n}")
}

/// The `Authorization` value each request carried, in order, empty where it carried none.
pub(crate) fn presented(received: &Received) -> Vec<String> {
    received
        .lock()
        .expect("received mutex poisoned")
        .iter()
        .map(|seen| seen.authorization.clone())
        .collect()
}

/// A response that closes its connection, so every request arrives on a connection of its own.
fn response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// Reads one whole request: its line, its headers, and its body, by `Content-Length` or in
/// chunks. `None` when the client closed before sending one.
fn read_request(stream: &std::net::TcpStream) -> Option<Seen> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let (mut authorization, mut length, mut chunked) = (String::new(), 0usize, false);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 || header == "\r\n" {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "authorization" => value.clone_into(&mut authorization),
            "content-length" => length = value.parse().unwrap_or(0),
            "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
            _ => {}
        }
    }
    let body = if chunked {
        read_chunks(&mut reader)?
    } else {
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).ok()?;
        body
    };
    Some(Seen {
        method,
        path,
        authorization,
        body: String::from_utf8_lossy_owned(body),
    })
}

/// A chunked body (RFC 9112 §7.1), to its last chunk and the blank line after it.
fn read_chunks(reader: &mut impl BufRead) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let mut size = String::new();
        reader.read_line(&mut size).ok()?;
        let size = usize::from_str_radix(size.trim().split(';').next()?, 16).ok()?;
        if size == 0 {
            let mut trailer = String::new();
            while reader.read_line(&mut trailer).ok()? > 2 {
                trailer.clear();
            }
            return Some(body);
        }
        let mut chunk = vec![0u8; size + 2];
        reader.read_exact(&mut chunk).ok()?;
        body.extend_from_slice(&chunk[..size]);
    }
}

/// Asserts that the server refused the first request's token and received the replacement on the
/// request sent again and on the next one, and that the token endpoint minted exactly those two.
pub(crate) fn assert_replaced_once(received: &Received, minted: &AtomicUsize) {
    assert_eq!(presented(received), [bearer(1), bearer(2), bearer(2)]);
    assert_eq!(minted.load(Ordering::SeqCst), 2);
}

/// The three answers [`assert_replaced_once`] expects: a refusal, then `body` twice.
pub(crate) fn refused_then(body: &str) -> Vec<Reply> {
    vec![
        Reply::RefusedBearer,
        Reply::Json(body.to_owned()),
        Reply::Json(body.to_owned()),
    ]
}
