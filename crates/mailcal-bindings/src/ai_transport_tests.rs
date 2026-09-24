use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

use mailcal_ai::{
    GatedBackend, HttpRequest, HttpTransport, Mode, OwnEndpoint,
    wire::{ChatMessage, ChatRequest, Purpose},
};

use super::AiTransport;

/// A one-request HTTP server on loopback: answers with `status` and `body`, and hands back the
/// request line, the headers and the body it received.
fn serve_once(status: &'static str, body: &'static str) -> (u16, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let mut received = vec![0; length];
        reader.read_exact(&mut received).unwrap();
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        head + &String::from_utf8(received).unwrap()
    });
    (port, handle)
}

fn request() -> ChatRequest {
    ChatRequest {
        purpose: Purpose::Draft,
        messages: vec![ChatMessage::user("hello")],
        tools: Vec::new(),
        tool_choice: None,
        temperature: None,
        max_tokens: None,
    }
}

/// The whole own-endpoint path over a real socket: the gate, the request shaper, this transport,
/// and the answer read back.
#[test]
fn an_own_endpoint_is_asked_over_http_and_its_answer_read() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (port, server) = serve_once(
        "200 OK",
        r#"{"choices":[{"message":{"role":"assistant","content":"Hi there"}}]}"#,
    );
    let backend = GatedBackend::own_endpoint(
        OwnEndpoint::new(
            &format!("http://127.0.0.1:{port}/v1"),
            Some("sk-local".to_owned()),
            "mistral-small",
            Some(mailcal_ai::Class::EuNative),
        )
        .unwrap(),
        Box::new(AiTransport::new(runtime.handle().clone()).unwrap()),
        Arc::new(|| Mode::EuNative),
    );

    let answer = backend.chat(&request()).unwrap();

    assert_eq!(
        answer.answer().unwrap().content.as_deref(),
        Some("Hi there")
    );
    let received = server.join().unwrap();
    assert!(received.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
    let lower = received.to_lowercase();
    assert!(lower.contains("authorization: bearer sk-local"));
    assert!(lower.contains("content-type: application/json"));
    assert!(received.contains(r#""model":"mistral-small""#));
    assert!(received.contains(r#""stream":false"#));
}

#[test]
fn an_error_status_is_an_answer_not_a_transport_failure() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (port, server) = serve_once("401 Unauthorized", "{}");
    let transport = AiTransport::new(runtime.handle().clone()).unwrap();
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");

    let answered = transport
        .post_json(HttpRequest {
            url: &url,
            bearer: None,
            body: "{}",
            timeout: Duration::from_secs(10),
        })
        .unwrap();

    assert_eq!(answered.status, 401);
    let received = server.join().unwrap();
    assert!(!received.to_lowercase().contains("authorization:"));
}

/// A server that accepts and never answers ends as a failure once the timeout passes, rather than
/// holding the caller for ever.
#[test]
fn a_server_that_never_answers_times_out() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let _held = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(5));
        drop(stream);
    });
    let transport = AiTransport::new(runtime.handle().clone()).unwrap();
    let url = format!("http://127.0.0.1:{port}/v1/chat/completions");

    let result = transport.post_json(HttpRequest {
        url: &url,
        bearer: None,
        body: "{}",
        timeout: Duration::from_millis(200),
    });

    assert!(result.is_err());
}

/// A server that takes a request and never answers it.
#[cfg(debug_assertions)]
fn serve_silence() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(30));
    });
    port
}

#[cfg(debug_assertions)]
#[test]
fn a_stop_abandons_a_request_that_is_waiting_on_an_answer() {
    use std::sync::atomic::{AtomicU64, Ordering};

    let stops: &'static AtomicU64 = Box::leak(Box::new(AtomicU64::new(0)));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let transport = AiTransport::stoppable(runtime.handle().clone(), stops).unwrap();
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", serve_silence());
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        stops.fetch_add(1, Ordering::Relaxed);
    });

    let started = std::time::Instant::now();
    let answered = transport.post_json(HttpRequest {
        url: &url,
        bearer: None,
        body: "{}",
        timeout: Duration::from_secs(20),
    });

    assert!(answered.is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
}
