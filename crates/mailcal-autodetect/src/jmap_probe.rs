//! The JMAP well-known probe: the highest-priority strategy, because this product is
//! JMAP-first.
//!
//! First an unauthenticated `GET https://{domain}/.well-known/jmap` (RFC 8620 §2.2). The
//! domain "speaks JMAP" when the endpoint either serves a JSON session object (an
//! anonymous session) or challenges for credentials (`401` + `WWW-Authenticate`, the
//! Stalwart/Fastmail shape). Every hop must be HTTPS: a positive proves an HTTPS JMAP
//! endpoint exists, so an HTTP hop is disqualifying (the dev-harness override waives
//! this to reach the local plaintext server).
//!
//! When the apex misses cleanly, the `_jmap._tcp.{domain}` SRV record is consulted; the
//! autodiscovery Fastmail publishes *instead* of an apex well-known (its apex `302`s to a
//! `404`, while `_jmap._tcp.fastmail.com` points at `api.fastmail.com`). Each SRV target
//! is probed the same way; a hit is trusted on the CA-validated TLS handshake alone (the
//! resolved host is pinned and re-validated on every connect), DNSSEC not required.
//!
//! A false positive (a wildcard `401` server) is bounded: the user lands on the
//! JMAP-prefilled form and the real connect fails honestly, one tap from manual setup.

use std::sync::Arc;

use crate::{
    DetectConfig,
    fetch::{Fetch, FetchOutcome, FetchResponse},
    mx,
    strategy::StrategyOutcome,
    types::{DetectedJmap, Domain, EmailParts, Source, SourceKind},
    urls,
};

/// Runs the probe for `email`'s domain: the apex well-known, then: only when that misses
/// cleanly and a resolver is available: the `_jmap._tcp` SRV target.
pub(crate) async fn run(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    resolver: Option<&Arc<dyn mx::MxResolver>>,
    config: &DetectConfig,
) -> StrategyOutcome {
    match probe_apex(fetcher, email, config).await {
        // A clean apex miss is the only case worth an SRV lookup; a hit or a transport
        // failure is returned as-is: so offline detection stays correct (we never reach
        // the SRV path after a network error).
        StrategyOutcome::Nothing => {}
        found_or_error => return found_or_error,
    }
    // The harness override rebases the probe onto a fixed local server; an SRV lookup is
    // meaningless there (and the typed domain can't be resolved anyway).
    match resolver {
        Some(resolver) if config.well_known_base_override.is_none() => {
            probe_via_srv(fetcher, email, resolver, config).await
        }
        _ => StrategyOutcome::Nothing,
    }
}

/// The apex probe: `https://{domain}/.well-known/jmap`, or the harness override base. A
/// positive must have stayed on HTTPS (waived only under the override).
async fn probe_apex(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    config: &DetectConfig,
) -> StrategyOutcome {
    let override_base = config.well_known_base_override.as_ref();
    let url = urls::jmap_well_known(&email.domain, override_base);

    let response = match fetcher.get(&url).await {
        FetchOutcome::Response(response) => response,
        FetchOutcome::Miss => return StrategyOutcome::Nothing,
        FetchOutcome::NetworkError => return StrategyOutcome::NetworkError,
    };

    // A positive must have stayed on HTTPS; the harness override waives that so a local
    // plaintext Stalwart is reachable.
    let https_ok = response.trusted || override_base.is_some();
    if !(https_ok && speaks_jmap(&response)) {
        return StrategyOutcome::Nothing;
    }

    // Store the domain base (production): the engine re-resolves /.well-known/jmap from
    // there, or the override origin (harness), which the typed domain can't reach.
    let base_url = match override_base {
        Some(base) => base.origin().ascii_serialization(),
        None => format!("https://{}", email.domain),
    };
    StrategyOutcome::Jmap(DetectedJmap {
        base_url,
        is_trusted: true,
        source: Source {
            kind: SourceKind::JmapWellKnown,
            url: response.final_url.to_string(),
        },
    })
}

/// The SRV fallback: resolve `_jmap._tcp.{domain}`, then probe each target's
/// `/.well-known/jmap` in priority order. The first target that speaks JMAP over HTTPS
/// wins, and is trusted: the endpoint was reached over CA-validated TLS (a plain-HTTP hop
/// is rejected above), which; since the resolved host is then pinned into the stored
/// config and re-validated on every connect, is what secures it. This deliberately trusts
/// the RFC 8620 cross-domain shape (a custom domain's `_jmap._tcp` pointing at the
/// provider, e.g. `example.fm` → `api.fastmail.com`) without DNSSEC; the residual one-time
/// setup-window risk is a documented known issue (see `docs/account-autodetect.md`), and
/// `resolution.authentic_data` stays available for a future opt-in "require DNSSEC" mode.
async fn probe_via_srv(
    fetcher: &dyn Fetch,
    email: &EmailParts,
    resolver: &Arc<dyn mx::MxResolver>,
    config: &DetectConfig,
) -> StrategyOutcome {
    let name = format!("_jmap._tcp.{}", email.domain);
    let Some(resolution) = mx::resolve_srv(resolver, &name, config.dns_timeout).await else {
        return StrategyOutcome::Nothing;
    };
    for record in mx::usable_srv_targets(&resolution) {
        // The target is DNS data, not a validated domain; parse it (dropping a trailing
        // dot, rejecting an IP literal) before it reaches a URL.
        let Some(target) = Domain::parse(&record.target) else {
            continue;
        };
        // A target on the email's own domain was already covered by the apex probe.
        if target == email.domain {
            continue;
        }
        let url = urls::jmap_well_known_at(&target, record.port);
        if let FetchOutcome::Response(response) = fetcher.get(&url).await
            && response.trusted
            && speaks_jmap(&response)
        {
            return StrategyOutcome::Jmap(DetectedJmap {
                base_url: url.origin().ascii_serialization(),
                is_trusted: true,
                source: Source {
                    kind: SourceKind::JmapSrv,
                    url: response.final_url.to_string(),
                },
            });
        }
    }
    StrategyOutcome::Nothing
}

/// Whether a terminal response indicates a JMAP endpoint: a `401` credential challenge,
/// or a 2xx JSON object advertising `capabilities`.
fn speaks_jmap(response: &FetchResponse) -> bool {
    if response.status == 401 && response.www_authenticate {
        return true;
    }
    response.is_success()
        && matches!(
            serde_json::from_slice::<serde_json::Value>(&response.body),
            Ok(serde_json::Value::Object(map)) if map.contains_key("capabilities")
        )
}

#[cfg(test)]
#[path = "jmap_probe_tests.rs"]
mod jmap_probe_tests;
