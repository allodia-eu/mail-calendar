//! Whether a server may be asked if it is Nextcloud at all. `status.php` is a PHP path, and a
//! server that runs no PHP can read a request for one as a probe for exploitable scripts:
//! Stalwart's default port-scanning rule bans the address that asks for `*.php*`, with no expiry,
//! and fail2ban-style rules do the same. So the question goes only to a server whose DAV endpoint
//! is itself a PHP script, which is how Nextcloud serves it and how SabreDAV is usually deployed
//! (`docs/nextcloud-sign-in.md` rule 1).

use reqwest::{Method, StatusCode};
use url::Url;

use crate::{Root, STATUS_TIMEOUT, Status, is_nextcloud, status_of};

/// Where a CalDAV client starts on a server it knows only by name (RFC 6764).
pub const WELL_KNOWN_PATH: &str = "/.well-known/caldav";

/// How many redirects the `.well-known` hop may take before it is given up.
const MAX_HOPS: usize = 3;

/// Whether the DAV endpoint at `url` is a PHP script: a path segment ending in `.php`, as
/// Nextcloud's `/remote.php/dav/` and SabreDAV's `/dav.php/` are.
#[must_use]
pub fn serves_php(url: &Url) -> bool {
    url.path_segments().is_some_and(|mut segments| {
        segments.any(|segment| {
            std::path::Path::new(segment)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("php"))
        })
    })
}

/// Whether the server at `url`, a DAV address as a person typed it or an account stores it,
/// answers as Nextcloud: [`nextcloud_root`] found one. Never an error.
pub async fn answers_as_nextcloud(client: &reqwest::Client, url: &str) -> bool {
    nextcloud_root(client, url).await.is_some()
}

/// The Nextcloud server the DAV address `url` reaches, when it answers as one. Its `status.php`
/// is asked only when its DAV endpoint is a PHP script: `url` itself, or, for an address that
/// names no endpoint (a bare host, or its `.well-known`), where the server's own
/// `.well-known/caldav` sends a client. That hop is asked without credentials and is the request
/// a connect makes first anyway. The root is the endpoint's, which for a bare host may be another
/// host's or a directory below it, so a sign-in runs where the question was answered. `None` for
/// any doubt.
pub async fn nextcloud_root(client: &reqwest::Client, url: &str) -> Option<Root> {
    let url = Url::parse(url.trim()).ok()?;
    // Nothing goes to an address no sign-in could run at: plain HTTP off this machine, or one
    // carrying credentials.
    Root::of(url.as_str())?;
    let endpoint = if serves_php(&url) {
        Some(url)
    } else {
        dav_endpoint(client, &url).await.filter(serves_php)
    };
    let Some(root) = endpoint.and_then(|endpoint| Root::of(endpoint.as_str())) else {
        log::debug!("nextcloud: the server shows no sign of being Nextcloud, so it is not asked");
        return None;
    };
    is_nextcloud(client, &root).await.then_some(root)
}

/// What asking an address typed as a Nextcloud server's found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    /// The server answers as Nextcloud, at this root.
    Nextcloud(Root),
    /// A Nextcloud server answered, in maintenance.
    Maintenance,
    /// The server is something else: a status document named another product, or a
    /// `.well-known` address led to a DAV server whose endpoint is no PHP script.
    Other,
    /// Nothing showed either way: no `.well-known` address led to a PHP endpoint, or no status
    /// document could be read. A Nextcloud server with calendars and contacts switched off can
    /// show the same.
    Unsure,
}

/// What the server at `address`, which a person typed as their Nextcloud server's (a host, or any
/// URL on it), shows of being one. The sign is [`nextcloud_root`]'s, looked for in more places: an
/// address that names no PHP endpoint asks the `.well-known/caldav` and then `.well-known/carddav`
/// of the directory it names, when it names one, and then of its host, and a redirect to a PHP
/// path is the sign even where that path then refuses. `status.php` is asked only at a root a
/// sign named, once each; the first that answers as Nextcloud is the server. A `.well-known`
/// address that leads to a DAV endpoint that is no PHP script shows another DAV server, which a
/// sign-in tried anyway would send a `.php` request to. A server that does not answer at all is
/// asked nothing more.
pub async fn named_nextcloud_root(client: &reqwest::Client, address: &str) -> Named {
    let Some(url) = Url::parse(address.trim())
        .ok()
        .filter(|url| Root::of(url.as_str()).is_some())
    else {
        return Named::Unsure;
    };
    if serves_php(&url) {
        let Some(root) = Root::of(url.as_str()) else {
            return Named::Unsure;
        };
        return match status_of(client, &root).await {
            Status::Nextcloud => Named::Nextcloud(root),
            Status::Maintenance => Named::Maintenance,
            Status::Other => Named::Other,
            Status::Unknown => Named::Unsure,
        };
    }
    let mut asked: Vec<(Root, Status)> = Vec::new();
    let mut dav_elsewhere = false;
    for start in well_known_starts(&url) {
        let (endpoint, landed) = match follow(client, start).await {
            Hop::Lands(endpoint) => (endpoint, true),
            Hop::Nowhere {
                via: Some(endpoint),
            } => (endpoint, false),
            Hop::Nowhere { via: None } => continue,
            Hop::Unreachable => break,
        };
        if !serves_php(&endpoint) {
            dav_elsewhere |= landed;
            continue;
        }
        let Some(root) = Root::of(endpoint.as_str()) else {
            continue;
        };
        if asked.iter().any(|(at, _)| *at == root) {
            continue;
        }
        match status_of(client, &root).await {
            Status::Nextcloud => return Named::Nextcloud(root),
            Status::Maintenance => return Named::Maintenance,
            status => asked.push((root, status)),
        }
    }
    let unknown = asked.iter().any(|(_, status)| *status == Status::Unknown);
    let other = dav_elsewhere || asked.iter().any(|(_, status)| *status == Status::Other);
    if other && !unknown {
        Named::Other
    } else {
        log::debug!("nextcloud: nothing showed whether the address is Nextcloud");
        Named::Unsure
    }
}

/// Where a typed Nextcloud address is asked for its DAV endpoint, in order: the `.well-known` of
/// the directory it names, when it names one, then its host's; calendar before contacts.
fn well_known_starts(url: &Url) -> Vec<Url> {
    let path = url.path();
    let directory = path
        .find("/.well-known")
        .map_or(path, |at| &path[..at])
        .trim_end_matches('/');
    let directories = if directory.is_empty() {
        vec![""]
    } else {
        vec![directory, ""]
    };
    directories
        .into_iter()
        .flat_map(|directory| {
            ["caldav", "carddav"].map(|service| {
                let mut start = url.clone();
                start.set_path(&format!("{directory}/.well-known/{service}"));
                start.set_query(None);
                start.set_fragment(None);
                start
            })
        })
        .collect()
}

/// Where `url` serves DAV, when it names no endpoint of its own: where its `.well-known/caldav`
/// lands, over any redirects, once it answers as DAV does (a challenge or a multi-status). `None`
/// for an address that names an endpoint already, which needs no hop, and for any doubt.
async fn dav_endpoint(client: &reqwest::Client, url: &Url) -> Option<Url> {
    let path = url.path();
    if !(path.is_empty() || path == "/" || path.starts_with("/.well-known/")) {
        return None;
    }
    let at = if path.starts_with("/.well-known/") {
        url.clone()
    } else {
        url.join(WELL_KNOWN_PATH).ok()?
    };
    match follow(client, at).await {
        Hop::Lands(endpoint) => Some(endpoint),
        Hop::Nowhere { .. } | Hop::Unreachable => None,
    }
}

/// Where following a `.well-known` address came to.
enum Hop {
    /// It answers as DAV does (a challenge or a multi-status) here, after any redirects.
    Lands(Url),
    /// The server answered, with anything else; `via` is the last address it redirected to.
    Nowhere { via: Option<Url> },
    /// The server did not answer.
    Unreachable,
}

/// Follows `start` over at most [`MAX_HOPS`] redirects with an unauthenticated `PROPFIND`.
async fn follow(client: &reqwest::Client, start: Url) -> Hop {
    let Ok(propfind) = Method::from_bytes(b"PROPFIND") else {
        return Hop::Nowhere { via: None };
    };
    let mut at = start.clone();
    let via = |at: &Url| (*at != start).then(|| at.clone());
    for _ in 0..=MAX_HOPS {
        let Ok(response) = client
            .request(propfind.clone(), at.clone())
            .header("Depth", "0")
            .timeout(STATUS_TIMEOUT)
            .send()
            .await
        else {
            // A redirect that led somewhere silent still showed where it led.
            return if at == start {
                Hop::Unreachable
            } else {
                Hop::Nowhere { via: via(&at) }
            };
        };
        let status = response.status();
        if !status.is_redirection() {
            return if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::MULTI_STATUS) {
                Hop::Lands(at)
            } else {
                Hop::Nowhere { via: via(&at) }
            };
        }
        let next = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|location| location.to_str().ok())
            .and_then(|location| at.join(location).ok())
            .map(|next| on_the_same_server(&at, next));
        match next {
            Some(next) => at = next,
            None => return Hop::Nowhere { via: via(&at) },
        }
    }
    Hop::Nowhere { via: via(&at) }
}

/// `next` as `from`'s server serves it: an `http` redirect on an HTTPS server, to the same host
/// and port, is answered as the `https` one, because a TLS-terminating proxy that does not tell
/// the server the scheme makes it write `http://` into every redirect it returns.
fn on_the_same_server(from: &Url, mut next: Url) -> Url {
    if from.scheme() == "https"
        && next.scheme() == "http"
        && next.host_str() == from.host_str()
        && next.port() == from.port()
    {
        // Infallible between two special schemes.
        let _ = next.set_scheme("https");
    }
    next
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod sign_tests;
