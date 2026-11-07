//! A loopback stand-in for a Nextcloud server, for the tests: it answers each request by its
//! method and path, so the DAV endpoint, `status.php`, the flow, OCS and remote wipe can all be
//! one server, and it records every request it read.

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use crate::{
    LogLevel, MailcalApp,
    tests::{
        ChannelObserver, NullLogger, RecordingCredentialStore, RecordingStoreHandle, temp_data_dir,
    },
};

/// A running stand-in.
pub(crate) struct MockNextcloud {
    /// `http://127.0.0.1:port`, the server's root.
    pub(crate) base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MockNextcloud {
    /// Every request read so far whose request line starts with `start` (`DELETE /ocs/`).
    pub(crate) fn requests_to(&self, start: &str) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with(start))
            .cloned()
            .collect()
    }

    /// Waits up to twenty seconds for a request starting with `start`.
    pub(crate) fn wait_for(&self, start: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(request) = self.requests_to(start).pop() {
                return request;
            }
            assert!(Instant::now() < deadline, "no `{start}` arrived");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

/// Serves `routes` until the test ends: the first whose `METHOD /path` prefix matches a request
/// answers it with its status and body, and anything else is `404`. A request line listed more
/// than once is answered by each in turn, the last for good. A route's body may name the server's
/// own base as `{base}`.
pub(crate) fn mock_nextcloud(routes: Vec<(&'static str, u16, &'static str)>) -> MockNextcloud {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);
    let own = base.clone();
    std::thread::spawn(move || {
        let mut routes = routes;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let request = read_request(&mut stream);
            recorded.lock().unwrap().push(request.clone());
            let matched = routes
                .iter()
                .position(|(start, ..)| request.starts_with(start));
            let (status, body) = matched.map_or((404, String::new()), |at| {
                let (start, status, body) = routes[at];
                if routes[at + 1..].iter().any(|(later, ..)| *later == start) {
                    routes.remove(at);
                }
                (status, body.replace("{base}", &own))
            });
            let mut head = format!(
                "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reason(status),
                body.len()
            );
            if status == 401 {
                head.push_str("WWW-Authenticate: Basic realm=\"Nextcloud\", charset=\"UTF-8\"\r\n");
            }
            let _ = stream.write_all(format!("{head}\r\n{body}").as_bytes());
        }
    });
    MockNextcloud { base, requests }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        207 => "Multi-Status",
        401 => "Unauthorized",
        _ => "Not Found",
    }
}

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

/// A calendar and contacts account a Nextcloud sign-in at `base` made, as the user `alice` signed
/// in as the opaque login `f3a9c1`.
pub(crate) fn nextcloud_account(base: &str) -> String {
    format!(
        "[caldav]\nbase_url = \"{base}/remote.php/dav/\"\nusername = \"f3a9c1\"\npassword = \"app-password\"\n\
         \n[nextcloud]\nserver = \"{base}\"\nuser = \"alice\"\naddress = \"alice@example.org\"\n"
    )
}

/// Boots an app over `configs`, returning it with the store it writes to.
pub(crate) fn boot(
    name: &str,
    configs: Vec<String>,
) -> (Arc<MailcalApp>, Arc<RecordingCredentialStore>) {
    let (tx, _rx) = mpsc::channel();
    let store = Arc::new(RecordingCredentialStore::default());
    let app = MailcalApp::new_accounts(
        Box::new(ChannelObserver { tx }),
        Box::new(NullLogger),
        LogLevel::Info,
        configs,
        temp_data_dir(name).to_string_lossy().into_owned(),
        "Etc/UTC".to_owned(),
        crate::analytics::test_device(),
        Box::new(RecordingStoreHandle(Arc::clone(&store))),
    )
    .expect("the app boots");
    (app, store)
}
