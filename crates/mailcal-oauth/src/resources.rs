//! Which endpoints an authorization request names, and which hosts a token may be sent to
//! (`draft-ietf-mailmaint-oauth-public` §3.4 and §3.6).
//!
//! The two are separate decisions. **Naming** an endpoint (the RFC 8707 `resource` parameter)
//! is done only where something from the issuer's side vouches for it: the issuer's
//! `protected_resources`, or the endpoint's own RFC 9728 metadata naming this issuer. A server
//! that supports resource indicators refuses an unknown one with `invalid_target`, so naming
//! by guess breaks the sign-in (Fastmail accepts its JMAP session URL and nothing else).
//! **Sending** a token is decided per host, re-checked after every redirect: a host the
//! issuer lists, or, with no list, a host on the issuer's own registrable domain.
//!
//! Neither decision trusts a resource's own claim to belong to an issuer. RFC 9728 metadata is
//! self-asserted: a hostile server can name any issuer. It is read only to find out where to
//! look, never as evidence a host may receive a token.

use crate::{AuthServerMetadata, DiscoveryError, discover_protected_resource};

/// Whether `entry` (a `protected_resources` value or a resource identifier) covers `endpoint`,
/// by §3.4's matching: for HTTP, identical scheme, host (case-insensitively) and port (the
/// scheme's default applied), and the entry's path a prefix of the endpoint's at a segment
/// boundary; for a non-HTTP endpoint, identical bare host names.
#[must_use]
pub fn resource_covers(entry: &str, endpoint: &str) -> bool {
    match (http_url(entry), http_url(endpoint)) {
        (Some(entry), Some(endpoint)) => {
            entry.scheme() == endpoint.scheme()
                && entry.host_str().map(str::to_ascii_lowercase)
                    == endpoint.host_str().map(str::to_ascii_lowercase)
                && entry.port_or_known_default() == endpoint.port_or_known_default()
                && entry.username().is_empty()
                && endpoint.username().is_empty()
                && path_prefix(entry.path(), endpoint.path())
        }
        (None, None) => entry.eq_ignore_ascii_case(endpoint),
        _ => false,
    }
}

/// The entry of `entries` covering `endpoint`, the longest path winning (§3.4).
#[must_use]
pub fn covering_entry(entries: &[String], endpoint: &str) -> Option<String> {
    entries
        .iter()
        .filter(|entry| resource_covers(entry, endpoint))
        .max_by_key(|entry| http_url(entry).map_or(0, |url| url.path().trim_end_matches('/').len()))
        .cloned()
}

/// Whether a token from `issuer` may be sent to `host` (§3.6): a host one of
/// `protected_resources` names, or, when the issuer lists none, a host with the issuer host's
/// registrable domain. A loopback issuer (a local test server) has no registrable domain, and
/// reaches its own host only.
#[must_use]
pub fn token_may_reach(issuer: &str, protected_resources: &[String], host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if !protected_resources.is_empty() {
        return protected_resources.iter().any(|entry| {
            let listed = http_url(entry)
                .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
                .unwrap_or_else(|| entry.to_ascii_lowercase());
            listed == host
        });
    }
    let Some(issuer_host) = http_url(issuer).and_then(|url| url.host_str().map(str::to_owned))
    else {
        return false;
    };
    match (registrable_domain(&issuer_host), registrable_domain(&host)) {
        (Some(issuer_domain), Some(domain)) => issuer_domain == domain,
        _ => issuer_host.eq_ignore_ascii_case(&host),
    }
}

/// The value to name `endpoint` by in an authorization request, or `None` to leave it unnamed.
///
/// The issuer's `protected_resources` decide when it lists any. Otherwise the endpoint is asked
/// for its RFC 9728 metadata, unauthenticated, and named by that document's `resource` only when
/// the document names this issuer and its `resource` covers the endpoint. A bare host name (an
/// IMAP or SMTP endpoint) has no metadata of its own, and is named only when the list has it.
///
/// # Errors
///
/// None that a caller acts on: an endpoint that cannot be asked is simply not named, and the
/// error is returned for the log.
pub async fn resource_for(
    http: &reqwest::Client,
    issuer: &AuthServerMetadata,
    endpoint: &str,
) -> Result<Option<String>, DiscoveryError> {
    if !issuer.protected_resources.is_empty() {
        return Ok(covering_entry(&issuer.protected_resources, endpoint));
    }
    if http_url(endpoint).is_none() {
        return Ok(None);
    }
    let described = discover_protected_resource(http, endpoint).await?;
    let vouched = described
        .authorization_servers
        .iter()
        .any(|server| server == &issuer.issuer);
    Ok(described
        .resource
        .filter(|resource| vouched && resource_covers(resource, endpoint)))
}

/// The registrable domain of `host` by the public suffix list, or `None` for a host that has
/// none (an IP address, `localhost`).
fn registrable_domain(host: &str) -> Option<String> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    psl::domain_str(&host).map(str::to_owned)
}

/// `value` as an `http`/`https` URL, or `None` for anything else (a bare host name).
fn http_url(value: &str) -> Option<url::Url> {
    url::Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

/// Whether `prefix`, without a trailing `/`, is `path` or is followed in it by `/`.
fn path_prefix(prefix: &str, path: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod resources_tests;
