//! Whether a server may be asked if it is Nextcloud at all. `status.php` is a PHP path, and a
//! server that runs no PHP can read a request for one as a probe for exploitable scripts:
//! Stalwart's default port-scanning rule bans the address that asks for `*.php*`, with no expiry,
//! and fail2ban-style rules do the same. So the question goes only to a server whose DAV endpoint
//! is itself a PHP script, which is how Nextcloud serves it and how SabreDAV is usually deployed
//! (`docs/nextcloud-sign-in.md` rule 1).

use reqwest::{Method, StatusCode};
use url::Url;

use crate::{Root, STATUS_TIMEOUT, is_nextcloud};

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

/// Where `url` serves DAV, when it names no endpoint of its own: where its `.well-known/caldav`
/// lands, over any redirects, once it answers as DAV does (a challenge or a multi-status). `None`
/// for an address that names an endpoint already, which needs no hop, and for any doubt.
async fn dav_endpoint(client: &reqwest::Client, url: &Url) -> Option<Url> {
    let path = url.path();
    if !(path.is_empty() || path == "/" || path.starts_with("/.well-known/")) {
        return None;
    }
    let mut at = if path.starts_with("/.well-known/") {
        url.clone()
    } else {
        url.join(WELL_KNOWN_PATH).ok()?
    };
    let propfind = Method::from_bytes(b"PROPFIND").ok()?;
    for _ in 0..=MAX_HOPS {
        let response = client
            .request(propfind.clone(), at.clone())
            .header("Depth", "0")
            .timeout(STATUS_TIMEOUT)
            .send()
            .await
            .ok()?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)?
                .to_str()
                .ok()?;
            at = at.join(location).ok()?;
            continue;
        }
        return matches!(status, StatusCode::UNAUTHORIZED | StatusCode::MULTI_STATUS).then_some(at);
    }
    None
}

#[cfg(test)]
#[path = "sign_tests.rs"]
mod sign_tests;
