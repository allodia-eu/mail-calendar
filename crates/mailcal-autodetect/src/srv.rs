//! The RFC 6186 / RFC 8314 mail SRV strategy: a domain that publishes `_imaps._tcp` /
//! `_imap._tcp` (and `_submissions._tcp` / `_submission._tcp`) SRV records names its mail
//! servers directly in DNS, with no autoconfig document. Both the **implicit-TLS** and the
//! **STARTTLS** service labels are queried (the engine speaks both) preferring implicit
//! TLS (RFC 8314's recommendation) and falling back to the STARTTLS label only when the
//! implicit-TLS one is absent. Plaintext is never assumed: a `_submission`/`_imap` target is
//! offered as STARTTLS, and the engine's connect **fails safe** (no cleartext downgrade) if
//! the server does not actually advertise `STARTTLS`.
//!
//! Because an SRV record names the very host a password is sent to, trust rests on the TLS
//! handshake: the endpoint is reached over CA-validated TLS, the resolved host is pinned
//! into the stored config, and its certificate re-validated on every connect: so a
//! discovered config is trusted on that alone, DNSSEC not required (the same stance as the
//! MX fallback), whether the upgrade is implicit or via STARTTLS. IMAP is required (no IMAP,
//! no mail config); Submission is best-effort.

use std::sync::Arc;

use crate::{
    DetectConfig, mx,
    strategy::StrategyOutcome,
    types::{
        AuthKind, DetectedMailSettings, DetectedServer, Domain, EmailParts, SocketKind, Source,
        SourceKind,
    },
};

/// The IMAP-over-implicit-TLS service label (RFC 6186), port 993.
const IMAPS_SERVICE: &str = "_imaps._tcp";
/// The IMAP-over-STARTTLS service label (RFC 6186), port 143.
const IMAP_SERVICE: &str = "_imap._tcp";
/// The Submission-over-implicit-TLS service label (RFC 8314), port 465.
const SUBMISSIONS_SERVICE: &str = "_submissions._tcp";
/// The Submission-over-STARTTLS service label (RFC 6186), port 587.
const SUBMISSION_SERVICE: &str = "_submission._tcp";

/// Runs the strategy for `email`'s domain: resolve the IMAP and Submission SRV records
/// (implicit TLS preferred, STARTTLS fallback) and, when IMAP is found, build a mail config.
pub(crate) async fn run(
    email: EmailParts,
    resolver: Arc<dyn mx::MxResolver>,
    config: DetectConfig,
) -> StrategyOutcome {
    let Some(incoming) =
        resolve_preferring_tls(&email, IMAPS_SERVICE, IMAP_SERVICE, &resolver, &config).await
    else {
        // No usable IMAP SRV; there is no mail config without an incoming server. (A
        // failed lookup is indistinguishable from "none advertised" here, so this is a
        // clean `Nothing`, not a network error, offline is caught by the other strategies.)
        return StrategyOutcome::Nothing;
    };
    let outgoing = resolve_preferring_tls(
        &email,
        SUBMISSIONS_SERVICE,
        SUBMISSION_SERVICE,
        &resolver,
        &config,
    )
    .await;

    StrategyOutcome::Mail(DetectedMailSettings {
        // An SRV record names hosts, not an authorization server: there is no document here
        // for a provider to describe itself in.
        oauth_issuer: None,
        incoming: vec![incoming],
        outgoing: outgoing.into_iter().collect(),
        // Trusted: SRV names servers the engine validates the certificate of on every connect
        // (implicit TLS or after the STARTTLS upgrade), and the resolved host is pinned into
        // the stored config, so DNS can't move it afterward. This trusts the RFC 6186
        // cross-domain shape without DNSSEC; the residual one-time setup-window risk is a
        // documented known issue, and the SRV answer's AD bit stays available (via
        // `resolve_srv`) for a future opt-in "require DNSSEC" mode.
        is_trusted: true,
        source: Source {
            kind: SourceKind::ImapSrv,
            url: format!("{IMAPS_SERVICE}.{}", email.domain),
        },
        // The orchestrator's RFC 6764 follow-on fills this; SRV mail records carry no
        // calendar endpoint, exactly as autoconfig/ISPDB don't.
        caldav_url: None,
    })
}

/// Resolves a mail service preferring the implicit-TLS label, falling back to the STARTTLS
/// label only when the implicit-TLS one names nothing usable (RFC 8314's preference order).
async fn resolve_preferring_tls(
    email: &EmailParts,
    tls_service: &str,
    starttls_service: &str,
    resolver: &Arc<dyn mx::MxResolver>,
    config: &DetectConfig,
) -> Option<DetectedServer> {
    if let Some(server) =
        resolve_server(email, tls_service, SocketKind::Tls, resolver, config).await
    {
        return Some(server);
    }
    resolve_server(
        email,
        starttls_service,
        SocketKind::StartTls,
        resolver,
        config,
    )
    .await
}

/// Resolves one mail service (`_imaps`/`_imap`/`_submissions`/`_submission`) for the domain
/// and turns its most-preferred usable target into a [`DetectedServer`] on the SRV-named
/// port with the given connection security. `None` when nothing usable resolves.
async fn resolve_server(
    email: &EmailParts,
    service: &str,
    socket: SocketKind,
    resolver: &Arc<dyn mx::MxResolver>,
    config: &DetectConfig,
) -> Option<DetectedServer> {
    let name = format!("{service}.{}", email.domain);
    let resolution = mx::resolve_srv(resolver, &name, config.dns_timeout).await?;
    let target = mx::usable_srv_targets(&resolution).into_iter().next()?;
    // The target is DNS data; validate it (dropping a trailing dot, rejecting an IP) into
    // a normalised hostname before it becomes a server we might connect.
    let host = Domain::parse(&target.target)?;
    let server = DetectedServer {
        hostname: host.as_str().to_owned(),
        port: target.port,
        // RFC 6186 SRV carries no auth hint, so assume a password over the (implicit or
        // STARTTLS-upgraded) TLS link; what `recommend` needs to route to the IMAP form.
        socket,
        auth: vec![AuthKind::PasswordCleartext],
        // RFC 6186 doesn't prescribe a username; the full address is the autoconfig
        // default (`%EMAILADDRESS%`) and what every provider expects.
        username: email.full.clone(),
    };
    Some(server)
}

#[cfg(test)]
#[path = "srv_tests.rs"]
mod srv_tests;
