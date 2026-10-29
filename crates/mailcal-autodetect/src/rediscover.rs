//! Looking for an account's calendar and address-book servers again, once the ones it stored
//! stop answering (RFC 6764 §6: cache what worked, and bootstrap again when it fails).
//!
//! The lookup is the same presence probe setup runs ([`crate::dav`]). What is new is the rule a
//! result must pass before a stored password follows it: DNS answers are not authenticated, so a
//! spoofed SRV record could otherwise point the credential anywhere (RFC 6764 §8). A server is
//! followed only under the registrable domain of the one it replaces or of the address itself.

use std::sync::Arc;

use crate::{
    DetectConfig, dav,
    fetch::{Fetch, Fetcher},
    mx,
    types::{DetectError, DetectedDav, Domain, EmailParts},
};

/// The calendar and address-book servers discovery finds for `email` now, looked for on its
/// domain and on the registrable domain of `server_host`, the host the account used until now.
/// `resolver` adds the SRV candidates; without one only the `.well-known` URLs are tried.
///
/// # Errors
///
/// Returns [`DetectError::InvalidEmail`] for an unparseable address, or
/// [`DetectError::Tls`] if the HTTP client cannot be built.
pub async fn discover_dav(
    email: &str,
    server_host: Option<&str>,
    resolver: Option<Arc<dyn mx::MxResolver>>,
    config: &DetectConfig,
) -> Result<DetectedDav, DetectError> {
    let fetcher = Fetcher::new(config)?;
    discover_with(&fetcher, email, server_host, resolver.as_ref(), config).await
}

pub(crate) async fn discover_with(
    fetcher: &dyn Fetch,
    email: &str,
    server_host: Option<&str>,
    resolver: Option<&Arc<dyn mx::MxResolver>>,
    config: &DetectConfig,
) -> Result<DetectedDav, DetectError> {
    let email = EmailParts::parse(email).ok_or(DetectError::InvalidEmail)?;
    let provider = server_host.and_then(dav::provider_domain);
    Ok(dav::probe(fetcher, resolver, &email.domain, provider.as_ref(), config).await)
}

/// The site of `url`: the registrable domain of its host (`icloud.com` for
/// `https://p01-caldav.icloud.com/…`), which is what a person can recognise and what
/// [`may_follow`] compares. `None` for a URL without a host the Public Suffix List can place.
#[must_use]
pub fn site_of(url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_owned();
    dav::provider_domain(&host).map(|domain| domain.as_str().to_owned())
}

/// Whether a stored credential may follow `candidate` in place of `stored` without asking: an
/// HTTPS URL whose host is under the registrable domain of `stored`'s host or of `email`'s
/// domain. A host the Public Suffix List cannot place (an address, `localhost`) is followed only
/// when it is the same host.
#[must_use]
pub fn may_follow(candidate: &str, stored: &str, email: &str) -> bool {
    let Ok(candidate) = url::Url::parse(candidate) else {
        return false;
    };
    let Some(host) = candidate
        .host_str()
        .filter(|_| candidate.scheme() == "https")
    else {
        return false;
    };
    let stored_host = url::Url::parse(stored)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    if stored_host.as_deref() == Some(host) {
        return true;
    }
    let Some(registrable) = dav::provider_domain(host) else {
        return false;
    };
    let stored_registrable = stored_host.as_deref().and_then(dav::provider_domain);
    let email_registrable = EmailParts::parse(email)
        .and_then(|email| dav::provider_domain(email.domain.as_str()))
        .or_else(|| EmailParts::parse(email).map(|email| email.domain));
    [stored_registrable, email_registrable]
        .iter()
        .flatten()
        .any(|domain: &Domain| *domain == registrable)
}

#[cfg(test)]
#[path = "rediscover_tests.rs"]
mod tests;
