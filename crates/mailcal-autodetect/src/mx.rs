//! The MX fallback: the lowest-priority strategy, and the one that makes custom
//! business domains work.
//!
//! When a domain publishes no autoconfig of its own, its MX record usually points at
//! the provider that actually hosts its mail (Google Workspace, Microsoft 365, a
//! webhost). This strategy asks the **host** to resolve MX (so the device's real DNS
//! settings apply: the core ships no resolver), takes the most-preferred record,
//! derives the provider's registrable domain via the Public Suffix List, and looks up
//! that provider's autoconfig/ISPDB (HTTPS only). The result is trusted on that validated
//! TLS fetch; DNSSEC is not required (the AD bit is still surfaced, reserved for a future
//! opt-in "require DNSSEC" setting): the same stance as the SRV strategies.

use std::{sync::Arc, time::Duration};

use tokio::task::JoinHandle;
use url::Url;

use crate::{
    DetectConfig,
    fetch::Fetch,
    strategy::{MailFetch, StrategyOutcome, fetch_mail_config},
    types::{Domain, EmailParts, SourceKind},
    urls,
};

/// The host-provided DNS resolver: each client answers with its native API so the
/// device's DNS configuration (VPN, private DNS) is honoured. Resolves both MX (the MX
/// fallback) and SRV (the JMAP and IMAP/SMTP autodiscovery records). Implementations run
/// on a blocking thread and may block.
pub trait MxResolver: Send + Sync {
    /// Resolves the MX records for `domain`.
    ///
    /// # Errors
    ///
    /// Returns [`MxError`] when the lookup fails; return an empty
    /// [`MxResolution::records`] for a clean "no MX records" answer.
    fn resolve_mx(&self, domain: &str) -> Result<MxResolution, MxError>;

    /// Resolves the SRV records for the owner name `name` (e.g.
    /// `_jmap._tcp.example.com`, `_imaps._tcp.example.com`); how a provider like Fastmail
    /// advertises a JMAP or IMAP/SMTP endpoint that isn't on the apex.
    ///
    /// # Errors
    ///
    /// Returns [`MxError`] when the lookup fails; return an empty
    /// [`SrvResolution::records`] for a clean "no SRV records" answer.
    fn resolve_srv(&self, name: &str) -> Result<SrvResolution, MxError>;
}

/// A completed MX lookup.
#[derive(Debug, Clone)]
pub struct MxResolution {
    /// The MX records (any order: the strategy takes the most preferred).
    pub records: Vec<MxRecord>,
    /// Whether the answer was DNSSEC-authenticated (the AD bit). Not used in any trust
    /// decision today (MX-derived results are trusted on the HTTPS fetch alone) and
    /// surfaced only for a future opt-in "require DNSSEC" setting.
    pub authentic_data: bool,
}

/// One MX record.
#[derive(Debug, Clone)]
pub struct MxRecord {
    /// The record's preference; lower is more preferred.
    pub preference: u16,
    /// The mail-exchange hostname.
    pub exchange: String,
}

/// A completed SRV lookup.
#[derive(Debug, Clone)]
pub struct SrvResolution {
    /// The SRV records (any order: the strategy sorts by priority).
    pub records: Vec<SrvRecord>,
    /// Whether the answer was DNSSEC-authenticated (the AD bit). Not used in any trust
    /// decision today: an SRV-discovered endpoint is trusted on the CA-validated TLS
    /// handshake alone, and surfaced only for a future opt-in "require DNSSEC" setting.
    pub authentic_data: bool,
}

/// One SRV record (RFC 2782): where a service lives, plus selection metadata.
#[derive(Debug, Clone)]
pub struct SrvRecord {
    /// Priority; lower is preferred (tried first).
    pub priority: u16,
    /// Weight for selection among equal priorities (not load-bearing for discovery).
    pub weight: u16,
    /// The TCP port the service listens on.
    pub port: u16,
    /// The target hostname. A single `.` means "the service is explicitly not offered".
    pub target: String,
}

/// A host DNS lookup failure. Treated as "no usable MX", never aborting the whole run.
#[derive(Debug, thiserror::Error)]
pub enum MxError {
    /// The lookup failed (no network, NXDOMAIN, timeout, malformed answer).
    #[error("mx lookup failed: {0}")]
    Lookup(String),
}

/// Runs the MX fallback for `email`'s domain.
pub(crate) async fn run(
    fetcher: Arc<dyn Fetch>,
    email: EmailParts,
    resolver: Arc<dyn MxResolver>,
    config: DetectConfig,
) -> StrategyOutcome {
    let Some(resolution) = resolve(&resolver, email.domain.as_str(), config.dns_timeout).await
    else {
        return StrategyOutcome::NetworkError;
    };
    let Some(exchange) = most_preferred(&resolution.records) else {
        return StrategyOutcome::Nothing; // resolved, but no MX records
    };
    let Some(base) = base_domain(&exchange) else {
        return StrategyOutcome::Nothing;
    };
    if base == email.domain {
        // The MX already lives on the email's own domain, nothing new to try.
        return StrategyOutcome::Nothing;
    }
    // Also try the MX host minus its first label, to tell (e.g.) Outlook.com from
    // Office365 business domains, but only when it differs from the base domain.
    let sub = sub_domain(&exchange).filter(|candidate| *candidate != base);

    let mut saw_response = false;
    for candidate in sub.into_iter().chain(std::iter::once(base)) {
        for url in urls::post_mx_urls(&candidate) {
            let kind = mx_source_kind(&url);
            match fetch_mail_config(fetcher.as_ref(), &url, kind, &email).await {
                // Trusted on the HTTPS fetch alone (post-MX URLs are HTTPS-only): the
                // provider's autoconfig servers are TLS-validated at connect, so; like SRV
                //; DNSSEC is not required. `resolution.authentic_data` stays available for a
                // future opt-in "require DNSSEC" mode.
                MailFetch::Found(settings) => return StrategyOutcome::Mail(settings),
                MailFetch::Miss => saw_response = true,
                MailFetch::NetworkError => {}
            }
        }
    }

    if saw_response {
        StrategyOutcome::Nothing
    } else {
        StrategyOutcome::NetworkError
    }
}

/// Resolves MX on a blocking thread, bounded by `timeout`. Any failure (error, timeout,
/// join failure) collapses to `None`; an empty-records success returns `Some`.
async fn resolve(
    resolver: &Arc<dyn MxResolver>,
    domain: &str,
    timeout: Duration,
) -> Option<MxResolution> {
    let (resolver, domain) = (Arc::clone(resolver), domain.to_owned());
    let handle = tokio::task::spawn_blocking(move || resolver.resolve_mx(&domain));
    await_lookup(handle, timeout, "mx").await
}

/// Resolves the SRV records for `name` on a blocking thread, bounded by `timeout`; the
/// shared plumbing behind the JMAP-SRV fallback and the IMAP/SMTP SRV strategy. Any
/// failure collapses to `None`; an empty-records success returns `Some`.
pub(crate) async fn resolve_srv(
    resolver: &Arc<dyn MxResolver>,
    name: &str,
    timeout: Duration,
) -> Option<SrvResolution> {
    let (resolver, name) = (Arc::clone(resolver), name.to_owned());
    let handle = tokio::task::spawn_blocking(move || resolver.resolve_srv(&name));
    await_lookup(handle, timeout, "srv").await
}

/// Awaits a blocking DNS-lookup task under `timeout`, collapsing every failure; lookup
/// error, join failure, or timeout; to `None`, logged at debug (`what` names the record
/// type). A clean empty-records success still returns `Some`.
async fn await_lookup<T>(
    handle: JoinHandle<Result<T, MxError>>,
    timeout: Duration,
    what: &str,
) -> Option<T> {
    match tokio::time::timeout(timeout, handle).await {
        Ok(Ok(Ok(resolution))) => Some(resolution),
        Ok(Ok(Err(err))) => {
            log::debug!("autodetect {what} lookup failed: {err}");
            None
        }
        Ok(Err(join)) => {
            log::debug!("autodetect {what} task failed: {join}");
            None
        }
        Err(_elapsed) => {
            log::debug!("autodetect {what} lookup timed out");
            None
        }
    }
}

/// The SRV records worth trying, most-preferred first: the RFC 2782 `.` "service not
/// offered" sentinel and any zero-port record are dropped, each surviving target is
/// normalised (the DNS trailing dot removed, so it parses as a [`crate::types::Domain`]),
/// and the rest are ordered by priority (weight-based load-balancing isn't meaningful for
/// one-shot discovery, where the first reachable target wins).
pub(crate) fn usable_srv_targets(resolution: &SrvResolution) -> Vec<SrvRecord> {
    let mut records: Vec<SrvRecord> = resolution
        .records
        .iter()
        .filter(|record| record.port != 0 && !is_no_service(&record.target))
        .map(|record| SrvRecord {
            target: record.target.trim().trim_end_matches('.').to_owned(),
            ..record.clone()
        })
        .collect();
    records.sort_by_key(|record| record.priority);
    records
}

/// Whether an SRV target is the RFC 2782 `.` sentinel: the provider saying the service
/// is deliberately not offered at this domain (Fastmail's `_submission._tcp`, for one).
fn is_no_service(target: &str) -> bool {
    let trimmed = target.trim().trim_end_matches('.');
    trimmed.is_empty()
}

/// The exchange of the most-preferred (lowest-preference) record, normalised: trailing
/// root dot removed and lowercased. Ties resolve to the first record (Thunderbird
/// parity).
fn most_preferred(records: &[MxRecord]) -> Option<String> {
    records
        .iter()
        .min_by_key(|record| record.preference)
        .map(|record| {
            record
                .exchange
                .trim()
                .trim_end_matches('.')
                .to_ascii_lowercase()
        })
        .filter(|exchange| !exchange.is_empty())
}

/// The registrable ("base") domain of `host` per the Public Suffix List, e.g.
/// `aspmx.l.google.com` → `google.com`.
fn base_domain(host: &str) -> Option<Domain> {
    Domain::parse(psl::domain_str(host)?)
}

/// `host` with its first label stripped, e.g. `mx.outlook-com.mail.protection.outlook.com`
/// → `outlook-com.mail.protection.outlook.com`.
fn sub_domain(host: &str) -> Option<Domain> {
    let (_, rest) = host.split_once('.')?;
    Domain::parse(rest)
}

/// Whether a post-MX URL is the ISPDB (versus a provider autoconfig endpoint).
fn mx_source_kind(url: &Url) -> SourceKind {
    if url.host_str() == Some("autoconfig.thunderbird.net") {
        SourceKind::MxIspdb
    } else {
        SourceKind::MxAutoconfig
    }
}

#[cfg(test)]
#[path = "mx_tests.rs"]
mod mx_tests;
