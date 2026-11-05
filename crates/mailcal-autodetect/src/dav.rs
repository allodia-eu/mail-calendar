//! The RFC 6764 CalDAV and CardDAV presence probe, run beside every route that found a server
//! and alone when nothing for mail was found.
//!
//! Mozilla autoconfig and the ISPDB describe **mail only**, and a JMAP server may not offer
//! calendars or contacts at all, so each service is looked for the unauthenticated way a client
//! bootstraps one (RFC 6764 §6), on up to two domains:
//!
//! - the account's **email domain**, for providers that host DAV on the custom domain (a
//!   self-hoster, Fastmail on your own domain);
//! - the **provider's registrable domain**, derived from the server that was found
//!   (`imap.soverin.net` → `soverin.net`, which advertises CalDAV even though the custom domain
//!   does not).
//!
//! On each domain the `_caldavs._tcp` / `_carddavs._tcp` SRV target's `.well-known` is tried
//! beside the domain's own. The preference is the SRV target, then the domain's `.well-known`,
//! the email domain before the provider's. Only HTTPS is followed and only a `401`/`207`
//! counts, so a discovered URL always comes from a tamper-resistant hop and a catch-all
//! `301`-to-homepage cannot be mistaken for a server. TXT `path` records (RFC 6764 §4) are not
//! read: the host resolver has no TXT lookup.
//!
//! The probe is **soft**: it never blocks or fails detection, every candidate is bounded by its
//! own timeout, and the engine does the real authenticated discovery at connect.

use std::sync::Arc;

use crate::{
    DetectConfig,
    fetch::{Fetch, FetchOutcome, FetchResponse},
    mx,
    types::{DetectedDav, Domain},
    urls,
};

/// One of the two DAV services, as RFC 6764 names it.
#[derive(Debug, Clone, Copy)]
enum Service {
    CalDav,
    CardDav,
}

impl Service {
    /// The `.well-known` name.
    const fn well_known(self) -> &'static str {
        match self {
            Self::CalDav => "caldav",
            Self::CardDav => "carddav",
        }
    }

    /// The SRV service label for the TLS service (RFC 6764 §3); the plaintext labels are never
    /// asked, since nothing would be sent to such a server.
    const fn srv_label(self) -> &'static str {
        match self {
            Self::CalDav => "_caldavs._tcp",
            Self::CardDav => "_carddavs._tcp",
        }
    }

    /// The domain's own `.well-known` URL.
    fn well_known_url(self, domain: &Domain) -> url::Url {
        match self {
            Self::CalDav => urls::caldav_well_known(domain),
            Self::CardDav => urls::carddav_well_known(domain),
        }
    }
}

/// The calendar and address-book servers found on `domain` and, when it differs, `provider`.
/// `resolver` adds the SRV candidates; without one only the `.well-known` URLs are tried.
pub(crate) async fn probe(
    fetcher: &dyn Fetch,
    resolver: Option<&Arc<dyn mx::MxResolver>>,
    domain: &Domain,
    provider: Option<&Domain>,
    config: &DetectConfig,
) -> DetectedDav {
    let provider = provider.filter(|provider| *provider != domain);
    let (caldav_url, carddav_url) = tokio::join!(
        probe_service(fetcher, resolver, Service::CalDav, domain, provider, config),
        probe_service(
            fetcher,
            resolver,
            Service::CardDav,
            domain,
            provider,
            config
        ),
    );
    DetectedDav {
        caldav_url,
        carddav_url,
        nextcloud: false,
    }
}

/// Whether the servers in `dav` are one Nextcloud installation: both endpoints under the same
/// root, one of them a PHP script, and that root's `status.php` saying so over HTTPS without being
/// redirected elsewhere. A server whose endpoints show no PHP is never asked a `.php` path, which
/// some servers answer by banning the address that asked (`docs/nextcloud-sign-in.md` rule 1).
/// Soft like the probe: any doubt is `false`, and the password route that follows works
/// everywhere.
pub(crate) async fn is_nextcloud(
    fetcher: &dyn Fetch,
    dav: &DetectedDav,
    config: &DetectConfig,
) -> bool {
    let endpoints: Vec<&String> = [&dav.caldav_url, &dav.carddav_url]
        .into_iter()
        .flatten()
        .collect();
    let php = endpoints
        .iter()
        .any(|url| url::Url::parse(url).is_ok_and(|url| mailcal_nextcloud::serves_php(&url)));
    if !php {
        return false;
    }
    let mut roots = endpoints
        .into_iter()
        .map(|url| mailcal_nextcloud::Root::of(url));
    let Some(Some(root)) = roots.next() else {
        return false;
    };
    if roots.any(|other| other.as_ref() != Some(&root)) {
        return false;
    }
    let Ok(status_url) = url::Url::parse(&root.endpoint(mailcal_nextcloud::STATUS_PATH)) else {
        return false;
    };
    let budget = mailcal_nextcloud::STATUS_TIMEOUT.min(config.http_timeout);
    match tokio::time::timeout(budget, fetcher.get(&status_url)).await {
        Ok(FetchOutcome::Response(response)) => {
            response.trusted
                && response.is_success()
                && response.final_url == status_url
                && mailcal_nextcloud::is_nextcloud_status(&response.body)
        }
        Ok(FetchOutcome::Miss | FetchOutcome::NetworkError) | Err(_) => false,
    }
}

/// The provider's registrable domain for a server `host` (`imap.soverin.net` →
/// `soverin.net`), via the Public Suffix List.
pub(crate) fn provider_domain(host: &str) -> Option<Domain> {
    Domain::parse(psl::domain_str(host)?)
}

/// One service on both domains, concurrently; the email domain's hit wins.
async fn probe_service(
    fetcher: &dyn Fetch,
    resolver: Option<&Arc<dyn mx::MxResolver>>,
    service: Service,
    domain: &Domain,
    provider: Option<&Domain>,
    config: &DetectConfig,
) -> Option<String> {
    let (own, provider) = tokio::join!(
        probe_domain(fetcher, resolver, service, domain, config),
        async {
            match provider {
                Some(provider) => probe_domain(fetcher, resolver, service, provider, config).await,
                None => None,
            }
        },
    );
    own.or(provider)
}

/// One service on one domain: the SRV target and the domain's own `.well-known`,
/// concurrently; the SRV target wins.
async fn probe_domain(
    fetcher: &dyn Fetch,
    resolver: Option<&Arc<dyn mx::MxResolver>>,
    service: Service,
    domain: &Domain,
    config: &DetectConfig,
) -> Option<String> {
    let own = service.well_known_url(domain);
    let (by_srv, well_known) = tokio::join!(
        async {
            match resolver {
                Some(resolver) => {
                    let budget = config.dns_timeout + config.http_timeout;
                    tokio::time::timeout(
                        budget,
                        probe_srv(fetcher, resolver, service, domain, config),
                    )
                    .await
                    .ok()
                    .flatten()
                }
                None => None,
            }
        },
        probe_url(fetcher, &own, config),
    );
    by_srv.or(well_known)
}

/// The `.well-known` of the most preferred target the domain's SRV record names, when it is
/// on another host than the domain itself (which the domain's own probe already covers).
async fn probe_srv(
    fetcher: &dyn Fetch,
    resolver: &Arc<dyn mx::MxResolver>,
    service: Service,
    domain: &Domain,
    config: &DetectConfig,
) -> Option<String> {
    let name = format!("{}.{domain}", service.srv_label());
    let resolution = mx::resolve_srv(resolver, &name, config.dns_timeout).await?;
    let record = mx::usable_srv_targets(&resolution).into_iter().next()?;
    // The target is DNS data; validate it (dropping a trailing dot, rejecting an IP) before
    // it reaches a URL.
    let target = Domain::parse(&record.target)?;
    if target == *domain && record.port == 443 {
        return None;
    }
    let url = urls::dav_well_known_at(&target, record.port, service.well_known());
    probe_url(fetcher, &url, config).await
}

/// Fetches one bootstrap URL; a DAV signal returns the endpoint (after any redirects the
/// fetcher followed).
async fn probe_url(fetcher: &dyn Fetch, url: &url::Url, config: &DetectConfig) -> Option<String> {
    match tokio::time::timeout(config.http_timeout, fetcher.propfind(url)).await {
        Ok(FetchOutcome::Response(response)) if is_dav(&response) => {
            Some(response.final_url.to_string())
        }
        Ok(FetchOutcome::Response(_) | FetchOutcome::Miss | FetchOutcome::NetworkError) => None,
        Err(_elapsed) => {
            log::debug!("autodetect dav probe timed out");
            None
        }
    }
}

/// Whether a terminal response indicates a DAV service: a credential challenge (`401`) or a
/// WebDAV multi-status (`207`), reached entirely over HTTPS. Both are signals a plain
/// redirect-to-website cannot fake.
fn is_dav(response: &FetchResponse) -> bool {
    response.trusted && matches!(response.status, 401 | 207)
}

#[cfg(test)]
#[path = "dav_tests.rs"]
mod dav_tests;
