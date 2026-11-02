//! Shared desktop OAuth loopback callback for provider and JMAP sign-in flows.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use gtk::gio;
use url::Url;

use crate::l10n;

pub(super) const CALLBACK_TIMEOUT: Duration = Duration::from_mins(5);
/// How long a port picked for one sign-in keeps answering once the wait is over.
const LINGERING: Duration = Duration::from_mins(3);
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
    /// Whether the port keeps answering once the wait is over. Only a port picked for this
    /// sign-in does. A persisted redirect's port must be free for the next re-authentication the
    /// moment this one ends, and a clone shares a socket the next wait accepts on, where a
    /// lingering answer could take that wait's redirect.
    lingers: bool,
}

impl OAuthLoopback {
    pub(super) fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
        Ok(Self {
            lingers: true,
            ..Self::from_listener(listener)?
        })
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
        Ok(Self {
            listener,
            address,
            lingers: false,
        })
    }

    pub(super) fn redirect_uri(&self) -> String {
        format!("http://{}/", self.address)
    }

    pub(super) fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            listener: self.listener.try_clone()?,
            address: self.address,
            lingers: false,
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
        let outcome = self.wait_until(
            cancel,
            timeout,
            expected_state,
            timeout_message,
            failure_message,
        );
        if self.lingers {
            self.linger(match outcome {
                CallbackOutcome::Received(_) => l10n::sign_in_page_done(),
                CallbackOutcome::Cancelled | CallbackOutcome::Failed(_) => {
                    l10n::sign_in_page_ended()
                }
            });
        }
        outcome
    }

    /// Answers every request with `message` for [`LINGERING`] once the wait is over, so a browser
    /// that comes back late (a sign-in cancelled or finished, or a tab reloaded) reads what
    /// happened rather than a refused connection.
    fn linger(self, message: &str) {
        let page = page(message);
        std::thread::spawn(move || {
            let until = Instant::now() + LINGERING;
            while Instant::now() < until {
                match self.listener.accept() {
                    Ok((mut stream, peer)) if peer.ip().is_loopback() => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                        let mut request = [0_u8; 1024];
                        let _ = stream.read(&mut request);
                        let _ = write_response(
                            &mut stream,
                            "200 OK",
                            "text/html; charset=utf-8",
                            &page,
                        );
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(ACCEPT_POLL);
                    }
                    Err(_) => return,
                }
            }
        });
    }

    fn wait_until(
        &self,
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
    mailcal_bindings::is_oauth_callback(target.to_owned()).then_some(target)
}

fn callback_state(target: &str) -> Option<&str> {
    target
        .split_once('?')?
        .1
        .split('&')
        .find_map(|part| part.strip_prefix("state="))
}

fn write_close_page(stream: &mut TcpStream) -> std::io::Result<()> {
    write_response(
        stream,
        "200 OK",
        "text/html; charset=utf-8",
        &page(l10n::sign_in_page_done()),
    )
}

/// A self-contained page saying the catalog's `message`, with no external assets so it renders
/// offline. The message carries the app's name, so it is escaped as text.
fn page(message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
         <body style=\"font-family: system-ui, sans-serif; text-align: center; padding: 3rem;\">\
         <p>{}</p></body></html>",
        escaped(l10n::app_title()),
        escaped(message)
    )
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
#[path = "oauth_loopback_tests.rs"]
mod tests;
