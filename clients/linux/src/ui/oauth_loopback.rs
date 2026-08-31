//! Shared desktop OAuth loopback callback for provider and JMAP sign-in flows.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use gtk::gio;
use url::Url;

pub(super) const CALLBACK_TIMEOUT: Duration = Duration::from_mins(5);
const ACCEPT_POLL: Duration = Duration::from_millis(50);
const MAX_REQUEST_BYTES: usize = 16 * 1024;

pub(super) enum CallbackOutcome {
    Received(String),
    Cancelled,
    Failed(String),
}

#[derive(Debug)]
pub(super) struct OAuthLoopback {
    listener: TcpListener,
    address: SocketAddrV4,
}

impl OAuthLoopback {
    pub(super) fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        Self::from_listener(listener)
    }

    /// Binds the exact loopback redirect persisted with a JMAP OAuth grant. A provider registers
    /// this URI, including its port, so re-authentication after a relaunch cannot choose a new one.
    pub(super) fn bind_redirect_uri(redirect_uri: &str) -> std::io::Result<Self> {
        let uri = Url::parse(redirect_uri)
            .map_err(|_| std::io::Error::other("invalid OAuth redirect URI"))?;
        if uri.scheme() != "http"
            || uri.host_str() != Some("127.0.0.1")
            || uri.path() != "/"
            || uri.query().is_some()
            || uri.fragment().is_some()
        {
            return Err(std::io::Error::other(
                "OAuth redirect URI is not an IPv4 loopback root",
            ));
        }
        let port = uri
            .port()
            .ok_or_else(|| std::io::Error::other("OAuth redirect URI has no port"))?;
        Self::from_listener(TcpListener::bind(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            port,
        ))?)
    }

    fn from_listener(listener: TcpListener) -> std::io::Result<Self> {
        listener.set_nonblocking(true)?;
        let address = match listener.local_addr()? {
            std::net::SocketAddr::V4(address) if address.ip().is_loopback() => address,
            _ => {
                return Err(std::io::Error::other(
                    "OAuth callback did not bind to IPv4 loopback",
                ));
            }
        };
        Ok(Self { listener, address })
    }

    pub(super) fn redirect_uri(&self) -> String {
        format!("http://{}/", self.address)
    }

    pub(super) fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            listener: self.listener.try_clone()?,
            address: self.address,
        })
    }

    pub(super) fn wait(
        self,
        cancel: &AtomicBool,
        timeout_message: &str,
        failure_message: &str,
    ) -> CallbackOutcome {
        self.wait_with_timeout(
            cancel,
            CALLBACK_TIMEOUT,
            None,
            timeout_message,
            failure_message,
        )
    }

    pub(super) fn wait_for_state(
        self,
        cancel: &AtomicBool,
        expected_state: &str,
        timeout_message: &str,
        failure_message: &str,
    ) -> CallbackOutcome {
        self.wait_with_timeout(
            cancel,
            CALLBACK_TIMEOUT,
            Some(expected_state),
            timeout_message,
            failure_message,
        )
    }

    fn wait_with_timeout(
        self,
        cancel: &AtomicBool,
        timeout: Duration,
        expected_state: Option<&str>,
        timeout_message: &str,
        failure_message: &str,
    ) -> CallbackOutcome {
        let deadline = Instant::now() + timeout;
        loop {
            if cancel.load(Ordering::Acquire) {
                return CallbackOutcome::Cancelled;
            }
            if Instant::now() >= deadline {
                return CallbackOutcome::Failed(timeout_message.to_owned());
            }
            match self.listener.accept() {
                Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                    if cancel.load(Ordering::Acquire) {
                        let _ = write_bad_request(&mut stream);
                        return CallbackOutcome::Cancelled;
                    }
                    if let Some(target) = request_target(&mut stream) {
                        if cancel.load(Ordering::Acquire) {
                            let _ = write_bad_request(&mut stream);
                            return CallbackOutcome::Cancelled;
                        }
                        if expected_state
                            .is_some_and(|expected| callback_state(&target) != Some(expected))
                        {
                            let _ = write_bad_request(&mut stream);
                            continue;
                        }
                        let _ = write_close_page(&mut stream);
                        return CallbackOutcome::Received(format!(
                            "http://{}{}",
                            self.address, target
                        ));
                    }
                    let _ = write_bad_request(&mut stream);
                }
                Ok((stream, _)) => drop(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL);
                }
                Err(_) => return CallbackOutcome::Failed(failure_message.to_owned()),
            }
        }
    }
}

/// Opens the authorisation page in the user's browser, and reports a failure to `on_error`.
///
/// **`GtkUriLauncher`, not `g_app_info_launch_default_for_uri`.** The latter is the host's API: it
/// resolves the URI against the desktop's application database, which a Flatpak sandbox does not
/// have, so GIO falls back through GVFS onto the session bus; and there it can wait for a reply
/// that never comes. It is also **synchronous**, and it is called from the GTK main thread, so a
/// wedged call takes the whole app with it: no repaint, and no Cancel button left to press. That is
/// exactly what a sandboxed build did, on the shape that ships.
///
/// `GtkUriLauncher` asks the **OpenURI portal**, which is what a sandboxed app is supposed to ask,
/// and it is asynchronous by construction; its callback lands back on the main context, so the
/// loop keeps running whatever the portal does.
///
/// Failure is therefore reported **later**, never as a return value. Each caller hands in the
/// closure that puts its own flow back into a failed state, because by the time it fires the
/// sign-in it belongs to has already started waiting for a redirect.
pub(super) fn launch_browser(authorization_url: &str, on_error: impl FnOnce() + 'static) {
    gtk::UriLauncher::new(authorization_url).launch(
        None::<&gtk::Window>,
        None::<&gio::Cancellable>,
        move |result| {
            if let Err(error) = result {
                // The message is the portal's, and names no account or URL.
                log::warn!("the browser could not be opened for a sign-in: {error}");
                on_error();
            }
        },
    );
}

fn request_target(stream: &mut TcpStream) -> Option<String> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    while request.len() < MAX_REQUEST_BYTES {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    if request.len() >= MAX_REQUEST_BYTES {
        return None;
    }
    parse_request_target(&request).map(str::to_owned)
}

fn parse_request_target(request: &[u8]) -> Option<&str> {
    let request = std::str::from_utf8(request).ok()?;
    let mut parts = request.lines().next()?.split_ascii_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let target = parts.next()?;
    if !target.starts_with('/')
        || parts
            .next()
            .is_none_or(|version| !version.starts_with("HTTP/"))
    {
        return None;
    }
    let query = target.split_once('?')?.1;
    let has_state = query.split('&').any(|part| part.starts_with("state="));
    let has_result = query
        .split('&')
        .any(|part| part.starts_with("code=") || part.starts_with("error="));
    (has_state && has_result).then_some(target)
}

fn callback_state(target: &str) -> Option<&str> {
    target
        .split_once('?')?
        .1
        .split('&')
        .find_map(|part| part.strip_prefix("state="))
}

fn write_close_page(stream: &mut TcpStream) -> std::io::Result<()> {
    const BODY: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Allodia Mail &amp; Calendar</title></head><body><p>You can close this tab and return to Allodia Mail &amp; Calendar.</p></body></html>";
    write_response(stream, "200 OK", "text/html; charset=utf-8", BODY)
}

fn write_bad_request(stream: &mut TcpStream) -> std::io::Result<()> {
    write_response(
        stream,
        "400 Bad Request",
        "text/plain; charset=utf-8",
        "Bad request",
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpStream,
        sync::{Arc, atomic::AtomicBool},
        time::Duration,
    };

    use super::{CallbackOutcome, OAuthLoopback, parse_request_target};

    #[test]
    fn loopback_accepts_exactly_an_oauth_callback_and_answers_locally() {
        let loopback = OAuthLoopback::bind().expect("bind loopback");
        let address = loopback.address;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker = std::thread::spawn(move || {
            loopback.wait_with_timeout(
                &worker_cancel,
                Duration::from_secs(2),
                None,
                "timeout",
                "failed",
            )
        });

        let mut browser = TcpStream::connect(address).expect("connect loopback");
        browser
            .write_all(b"GET /?code=fixture&state=expected HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("write callback");
        let mut response = String::new();
        browser
            .read_to_string(&mut response)
            .expect("read response");

        let CallbackOutcome::Received(url) = worker.join().expect("worker") else {
            panic!("expected callback");
        };
        assert_eq!(
            url,
            format!("http://{address}/?code=fixture&state=expected")
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Allodia Mail &amp; Calendar"));
    }

    #[test]
    fn a_reauthentication_rebinds_the_exact_persisted_loopback_uri() {
        let original = OAuthLoopback::bind().expect("bind original loopback");
        let redirect = original.redirect_uri();
        drop(original);

        let rebound = OAuthLoopback::bind_redirect_uri(&redirect).expect("rebind persisted port");

        assert_eq!(rebound.redirect_uri(), redirect);
        assert!(OAuthLoopback::bind_redirect_uri("http://example.com:1234/").is_err());
        assert!(OAuthLoopback::bind_redirect_uri("http://127.0.0.1:1234/not-root").is_err());
    }

    #[test]
    fn loopback_parser_rejects_non_callbacks() {
        assert!(parse_request_target(b"GET /favicon.ico HTTP/1.1\r\n\r\n").is_none());
        assert!(parse_request_target(b"POST /?code=x&state=y HTTP/1.1\r\n\r\n").is_none());
        assert_eq!(
            parse_request_target(b"GET /?error=access_denied&state=y HTTP/1.1\r\n\r\n"),
            Some("/?error=access_denied&state=y")
        );
    }

    #[test]
    fn cancelling_stops_the_loopback_wait() {
        let loopback = OAuthLoopback::bind().expect("bind loopback");
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            loopback.wait_with_timeout(&cancel, Duration::from_secs(2), None, "timeout", "failed",),
            CallbackOutcome::Cancelled
        ));
    }

    #[test]
    fn state_filtered_wait_ignores_a_late_callback_from_a_cancelled_attempt() {
        let owner = OAuthLoopback::bind().expect("bind loopback");
        let address = owner.address;
        let loopback = owner.try_clone().expect("clone listener");
        assert_eq!(owner.redirect_uri(), loopback.redirect_uri());
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker = std::thread::spawn(move || {
            loopback.wait_with_timeout(
                &worker_cancel,
                Duration::from_secs(2),
                Some("current"),
                "timeout",
                "failed",
            )
        });

        let stale = request(address, "/?code=old&state=cancelled");
        assert!(stale.starts_with("HTTP/1.1 400 Bad Request"));
        let current = request(address, "/?code=new&state=current");
        assert!(current.starts_with("HTTP/1.1 200 OK"));

        let CallbackOutcome::Received(url) = worker.join().expect("worker") else {
            panic!("expected current callback");
        };
        assert_eq!(url, format!("http://{address}/?code=new&state=current"));
    }

    fn request(address: std::net::SocketAddrV4, target: &str) -> String {
        let mut browser = TcpStream::connect(address).expect("connect loopback");
        write!(browser, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("write callback");
        let mut response = String::new();
        browser
            .read_to_string(&mut response)
            .expect("read response");
        response
    }
}
