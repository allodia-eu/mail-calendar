//! Where a Nextcloud server is installed, and which URLs belong to it.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

/// The address a Nextcloud server is installed at: its origin, plus the directory when it is
/// installed in one (`https://example.org/nextcloud`). Every endpoint this crate calls hangs off
/// it, and it is the only server an app password it obtained is ever sent to.
///
/// HTTPS, or HTTP to this machine for a local test server.
#[derive(Clone, PartialEq, Eq)]
pub struct Root {
    url: Url,
    text: String,
}

/// The paths a Nextcloud URL may carry below its installation directory. A URL naming one is
/// cut where it starts, which is how a DAV endpoint, a `.well-known` hop or a login page all name
/// the same server.
const BELOW_ROOT: [&str; 5] = [
    "/remote.php/",
    "/.well-known/",
    "/index.php",
    "/status.php",
    "/ocs/",
];

impl Root {
    /// The server `url` belongs to, or `None` when it cannot be one: not HTTPS (and not HTTP to
    /// this machine), carrying a user name or password, or not a URL at all. Anything below the
    /// installation directory is dropped, so `https://cloud.example.org/remote.php/dav/` and
    /// `https://cloud.example.org/` are the same server.
    #[must_use]
    pub fn of(url: &str) -> Option<Self> {
        let mut url = Url::parse(url.trim()).ok()?;
        let permitted = match url.scheme() {
            "https" => true,
            "http" => is_loopback(&url),
            _ => false,
        };
        if !permitted
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return None;
        }
        let path = url.path().to_owned();
        let cut = BELOW_ROOT
            .iter()
            .filter_map(|marker| path.find(marker))
            .min()
            .map_or(path.as_str(), |at| &path[..at]);
        let cut = cut.trim_end_matches('/');
        let cut = cut.strip_suffix("/remote.php").unwrap_or(cut);
        url.set_path(cut);
        url.set_query(None);
        url.set_fragment(None);
        let text = url.as_str().trim_end_matches('/').to_owned();
        Some(Self { url, text })
    }

    /// The root as text, without a trailing slash: `https://cloud.example.org`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The server's host, for showing a person which server this is.
    #[must_use]
    pub fn host(&self) -> &str {
        self.url.host_str().unwrap_or_default()
    }

    /// The URL of `path` on this server; `path` starts with `/`.
    #[must_use]
    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{path}", self.text)
    }

    /// The DAV endpoint Nextcloud serves calendars and address books under, for both.
    #[must_use]
    pub fn dav_url(&self) -> String {
        self.endpoint("/remote.php/dav/")
    }

    /// `url` when it is on this server, the same host and port; an `http` URL on an HTTPS server
    /// is answered as the `https` one. A TLS-terminating proxy that does not tell Nextcloud the
    /// scheme makes it write `http://` into every absolute URL it returns, and the server behind
    /// the proxy is still this one. `None` for anything else, so no URL a server returns can send
    /// a token or a person anywhere but here.
    #[must_use]
    pub fn owns(&self, url: &str) -> Option<String> {
        let mut url = Url::parse(url).ok()?;
        if url.host_str() != self.url.host_str()
            || url.port() != self.url.port()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return None;
        }
        match (self.url.scheme(), url.scheme()) {
            (ours, theirs) if ours == theirs => {}
            ("https", "http") => url.set_scheme("https").ok()?,
            _ => return None,
        }
        Some(url.into())
    }
}

impl fmt::Debug for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Root({})", self.text)
    }
}

impl fmt::Display for Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl Serialize for Root {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.text)
    }
}

impl<'de> Deserialize<'de> for Root {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::of(&text).ok_or_else(|| serde::de::Error::custom("not a Nextcloud server address"))
    }
}

/// Whether `url`'s host is this machine, and therefore off any network. Host-based, so
/// `http://127.0.0.1.example.com/` is not.
fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

#[cfg(test)]
#[path = "root_tests.rs"]
mod root_tests;
