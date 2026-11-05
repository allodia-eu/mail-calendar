//! A loopback HTTP server for the tests: it answers canned responses in order, one per
//! connection, and records every request it read, so a test can assert on what was sent.

use std::{
    fmt::Write as _,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

/// A running server.
pub(crate) struct Server {
    /// `http://127.0.0.1:port`.
    pub base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    /// Every request read so far, head and body, in order.
    pub(crate) fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

/// Serves `responses` in order, then stops accepting.
pub(crate) fn serve(responses: Vec<String>) -> Server {
    serve_with(|_| responses)
}

/// Serves what `responses` writes for the server's own base URL, for answers that name it.
pub(crate) fn serve_with(responses: impl FnOnce(&str) -> Vec<String>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let responses = responses(&base);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    thread::spawn(move || {
        for reply in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let request = read_request(&mut stream);
            recorded.lock().unwrap().push(request);
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.flush();
        }
    });
    Server { base, requests }
}

/// Reads one request: the head, then as much body as its `Content-Length` names.
fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok(read) = stream.read(&mut buf) {
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buf[..read]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

/// A response with `status_line` (`200 OK`), extra headers and `body`.
pub(crate) fn response(status_line: &str, headers: &[(&str, &str)], body: &str) -> String {
    let mut head = format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    format!("{head}\r\n{body}")
}

/// A JSON response with `status`.
pub(crate) fn json(status: u16, body: &str) -> String {
    let line = match status {
        200 => "200 OK",
        404 => "404 Not Found",
        401 => "401 Unauthorized",
        403 => "403 Forbidden",
        500 => "500 Internal Server Error",
        _ => "418 I'm a teapot",
    };
    response(line, &[("Content-Type", "application/json")], body)
}

/// The client the tests call through.
pub(crate) fn client() -> reqwest::Client {
    crate::client_for(&engine_tls::TlsPolicy::bundled()).unwrap()
}
