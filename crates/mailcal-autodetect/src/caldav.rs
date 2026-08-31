//! The RFC 6764 CalDAV presence probe: a follow-on once a mail config is found.
//!
//! Mozilla autoconfig and the ISPDB describe **mail only**; neither carries a calendar
//! endpoint. So after mail settings are found, this probes for a CalDAV service the same
//! unauthenticated way a client bootstraps one: `GET https://{domain}/.well-known/caldav`
//! (RFC 6764 §5). A present service redirects that to its context path or answers `401`;
//! an absent one `404`s.
//!
//! Two domains are tried, concurrently:
//!
//! - the account's **email domain**: for providers that host CalDAV on the custom domain (a
//!   self-hoster, Fastmail-on-your-own-domain);
//! - the **provider's registrable domain**, derived from the winning IMAP host: for the common case
//!   where mail actually lives on a provider (`imap.soverin.net` → `soverin.net`, which advertises
//!   CalDAV even though the custom domain does not).
//!
//! The email-domain hit wins a tie. Only HTTPS is followed and only a `401`/`207` counts,
//! so a discovered URL always comes from a tamper-resistant hop and a catch-all
//! `301`-to-homepage can't be mistaken for a calendar. Calendar is a **soft** add-on:
//! this never blocks or fails mail detection, it is bounded by its own timeout, and the
//! engine does the real authenticated collection discovery at connect.

use std::time::Duration;

use crate::{
    fetch::{Fetch, FetchOutcome, FetchResponse},
    types::{DetectedMailSettings, Domain, EmailParts},
    urls,
};

/// Probes for a CalDAV endpoint for a found mail config, returning the discovered URL (the
/// RFC 6764 landing point) or `None`. Bounded by `budget`, so a black-hole host can only
/// cost mail detection a fixed, small delay before calendar is quietly skipped.
pub(crate) async fn probe(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    settings: &DetectedMailSettings,
    budget: Duration,
) -> Option<String> {
    match tokio::time::timeout(budget, probe_candidates(fetcher, email, settings)).await {
        Ok(found) => found,
        Err(_elapsed) => {
            log::debug!("autodetect caldav probe timed out");
            None
        }
    }
}

/// Probes the email domain and the provider domain concurrently; the email-domain hit
/// wins a tie. The provider domain is skipped when it is absent or equals the email
/// domain (the same `.well-known` already covered).
async fn probe_candidates(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    settings: &DetectedMailSettings,
) -> Option<String> {
    let provider = provider_domain(settings).filter(|domain| *domain != email.domain);
    let (email_hit, provider_hit) = tokio::join!(
        probe_domain(fetcher, &email.domain),
        maybe_probe(fetcher, provider.as_ref()),
    );
    email_hit.or(provider_hit)
}

/// Probes `domain` when it is `Some`, else resolves to `None` without a request.
async fn maybe_probe(fetcher: &dyn Fetch, domain: Option<&Domain>) -> Option<String> {
    match domain {
        Some(domain) => probe_domain(fetcher, domain).await,
        None => None,
    }
}

/// Probes one domain's `.well-known/caldav`; a positive returns the endpoint URL (after
/// any redirects the fetcher followed).
async fn probe_domain(fetcher: &dyn Fetch, domain: &Domain) -> Option<String> {
    match fetcher.get(&urls::caldav_well_known(domain)).await {
        FetchOutcome::Response(response) if is_caldav(&response) => {
            Some(response.final_url.to_string())
        }
        FetchOutcome::Response(_) | FetchOutcome::Miss | FetchOutcome::NetworkError => None,
    }
}

/// Whether a terminal response indicates a CalDAV service: a credential challenge (`401`)
/// or a WebDAV multi-status (`207`), reached entirely over HTTPS. Both are signals a plain
/// redirect-to-website cannot fake, so a catch-all `301` is not a false positive.
fn is_caldav(response: &FetchResponse) -> bool {
    response.trusted && matches!(response.status, 401 | 207)
}

/// The provider's registrable domain, from the preferred incoming server's host (e.g.
/// `imap.soverin.net` → `soverin.net`), via the Public Suffix List. `incoming` is never
/// empty for a found config.
fn provider_domain(settings: &DetectedMailSettings) -> Option<Domain> {
    let host = &settings.incoming.first()?.hostname;
    Domain::parse(psl::domain_str(host)?)
}

#[cfg(test)]
#[path = "caldav_tests.rs"]
mod caldav_tests;
