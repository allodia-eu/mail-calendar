//! The shared vocabulary of a lookup strategy and the fetch-then-parse helper the
//! autoconfig, ISPDB, and MX strategies all use to turn one URL into mail settings.

use url::Url;

use crate::{
    fetch::{Fetch, FetchOutcome},
    parser::parse_autoconfig,
    types::{DetectedJmap, DetectedMailSettings, EmailParts, Source, SourceKind},
    urls,
};

/// What one strategy resolved to. The orchestrator collects these in priority order.
#[derive(Debug)]
pub(crate) enum StrategyOutcome {
    /// The domain speaks JMAP.
    Jmap(DetectedJmap),
    /// IMAP/SMTP settings were found.
    Mail(DetectedMailSettings),
    /// This strategy cleanly found nothing.
    Nothing,
    /// This strategy failed on transport (offline-ish): no usable answer.
    NetworkError,
}

/// The three ways fetching-and-parsing one autoconfig URL can end.
pub(crate) enum MailFetch {
    /// A valid config parsed into these settings.
    Found(DetectedMailSettings),
    /// A response came back but held no usable config (non-2xx, unparseable, capped).
    Miss,
    /// The request failed on transport.
    NetworkError,
}

/// Fetches `url`, and if it is a 2xx autoconfig document, parses it into settings tagged
/// with `kind` and the fetch's trust. A non-2xx response or an unparseable body is a
/// clean [`MailFetch::Miss`]; a transport failure is [`MailFetch::NetworkError`].
pub(crate) async fn fetch_mail_config(
    fetcher: &dyn Fetch,
    url: &Url,
    kind: SourceKind,
    email: &EmailParts,
) -> MailFetch {
    match fetcher.get(url).await {
        FetchOutcome::Response(response) if response.is_success() => {
            match parse_autoconfig(&response.body, email) {
                Ok(servers) => MailFetch::Found(DetectedMailSettings {
                    incoming: servers.incoming,
                    outgoing: servers.outgoing,
                    // Only a provider describing **itself**, over HTTPS, may name an
                    // authorization server (`docs/account-autodetect.md` rule 7).
                    oauth_issuer: servers
                        .oauth_issuer
                        .filter(|_| response.trusted && describes_itself(kind)),
                    is_trusted: response.trusted,
                    source: Source {
                        kind,
                        url: response.final_url.to_string(),
                    },
                    // Filled in later by the CalDAV follow-on probe in the orchestrator;
                    // mail settings alone carry no calendar endpoint.
                    caldav_url: None,
                }),
                Err(err) => {
                    log::debug!("autodetect could not parse {}: {err}", response.final_url);
                    MailFetch::Miss
                }
            }
        }
        FetchOutcome::Response(_) | FetchOutcome::Miss => MailFetch::Miss,
        FetchOutcome::NetworkError => MailFetch::NetworkError,
    }
}

/// Whether a document from this source is the provider **describing itself**, and so may
/// name the authorization server a user will be sent to sign in at.
///
/// The provider's own autoconfig endpoints qualify; the ISPDB does not. The distinction is
/// not about how carefully Mozilla curates that database: an issuer decides which page
/// receives someone's password, and a third party naming it for a provider is a different
/// trust decision from the provider naming it for itself. The MX-derived autoconfig
/// qualifies because it is still the mail provider's own endpoint, reached one DNS hop away.
///
/// The endpoints beside the issuer are never taken from any source; only the issuer's own
/// RFC 8414 metadata says what they are.
const fn describes_itself(kind: SourceKind) -> bool {
    match kind {
        SourceKind::Autoconfig | SourceKind::AutoconfigWellKnown | SourceKind::MxAutoconfig => true,
        SourceKind::Ispdb
        | SourceKind::MxIspdb
        | SourceKind::ImapSrv
        | SourceKind::JmapWellKnown
        | SourceKind::JmapSrv => false,
    }
}

/// The autoconfig strategy for `email`'s own domain: the four provider URLs (HTTPS
/// then HTTP), first parseable config wins.
pub(crate) async fn run_autoconfig(fetcher: &dyn Fetch, email: &EmailParts) -> StrategyOutcome {
    let candidates = urls::autoconfig_urls(&email.domain)
        .into_iter()
        .map(|url| (autoconfig_kind(&url), url));
    run_candidates(fetcher, candidates, email).await
}

/// The ISPDB strategy: one lookup of Thunderbird's provider database by domain.
pub(crate) async fn run_ispdb(fetcher: &dyn Fetch, email: &EmailParts) -> StrategyOutcome {
    let url = urls::ispdb_url(&email.domain);
    run_candidates(fetcher, std::iter::once((SourceKind::Ispdb, url)), email).await
}

/// Tries each `(kind, url)` in order; the first parseable config wins. Reports
/// [`StrategyOutcome::NetworkError`] only when every candidate failed on transport.
async fn run_candidates(
    fetcher: &dyn Fetch,
    candidates: impl Iterator<Item = (SourceKind, Url)>,
    email: &EmailParts,
) -> StrategyOutcome {
    let mut all_network = true;
    for (kind, url) in candidates {
        match fetch_mail_config(fetcher, &url, kind, email).await {
            MailFetch::Found(settings) => return StrategyOutcome::Mail(settings),
            MailFetch::Miss => all_network = false,
            MailFetch::NetworkError => {}
        }
    }
    if all_network {
        StrategyOutcome::NetworkError
    } else {
        StrategyOutcome::Nothing
    }
}

/// Whether a same-domain autoconfig URL is the `autoconfig.{domain}` host or the
/// `/.well-known/autoconfig` path.
fn autoconfig_kind(url: &Url) -> SourceKind {
    if url
        .host_str()
        .is_some_and(|host| host.starts_with("autoconfig."))
    {
        SourceKind::Autoconfig
    } else {
        SourceKind::AutoconfigWellKnown
    }
}

#[cfg(test)]
#[path = "strategy_tests.rs"]
mod strategy_tests;
