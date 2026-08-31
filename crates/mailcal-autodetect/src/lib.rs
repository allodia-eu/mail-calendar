//! Automatic mail-server-settings detection from an email address.
//!
//! A user types `alice@company.example`; this crate races several lookup strategies and
//! returns the best discovered configuration (or a clean "nothing found"):
//!
//! | Priority | Strategy | What it asks |
//! |---|---|---|
//! | 0 | JMAP probe | `https://{domain}/.well-known/jmap`, then the `_jmap._tcp` SRV target; does the domain speak JMAP? |
//! | 1 | Autoconfig | `autoconfig.{domain}` + `/.well-known/autoconfig/…` (HTTPS, then HTTP) |
//! | 2 | ISPDB | Thunderbird's provider database, by domain only |
//! | 3 | IMAP/SMTP SRV | `_imaps._tcp` + `_submissions._tcp` (RFC 6186/8314, implicit TLS only) |
//! | 4 | MX fallback | host-resolved MX → provider's registrable domain → autoconfig + ISPDB |
//!
//! All strategies start at once; the **lowest-priority-number success wins**, a
//! lower-priority success waits for every higher strategy to finish, and losers are
//! cancelled: the model of Thunderbird for Android's autodiscovery, reimplemented in
//! the shared core so every client gets it from one implementation (extended with the
//! SRV strategies, which Thunderbird omits).
//!
//! Two properties are load-bearing (and bind every caller, see
//! `docs/account-autodetect.md`):
//!
//! - **Trust**: a result [`is_trusted`](DetectedMailSettings::is_trusted) only if every fetch hop
//!   was HTTPS (CA-validated TLS). DNS-derived results (MX and SRV) are trusted on that TLS alone;
//!   DNSSEC is not required, since the resolved host is pinned into the stored config and its
//!   certificate re-validated on every connect. The only untrusted case is a non-HTTPS hop.
//!   Untrusted settings must be explicitly approved by the user before any credential is sent to
//!   the servers they name.
//! - **Privacy**: the email address never appears in any URL, only the domain (and an SRV target
//!   host) is disclosed; to the provider's own endpoints, the ISPDB, and the DNS resolver. Failures
//!   are silent skips, logged at debug level without the local part.
//!
//! DNS is deliberately **not** resolved in Rust: the strategies take a host-provided
//! resolver so each platform's native API (and thus the device's real DNS settings;
//! VPNs, private DNS) answers the MX and SRV queries.

mod caldav;
mod fetch;
mod hostname;
mod jmap_probe;
mod mx;
mod orchestrator;
mod parser;
mod srv;
mod strategy;
mod types;
pub mod urls;

#[cfg(test)]
mod test_fakes;

use std::time::Duration;

pub use mx::{MxError, MxRecord, MxResolution, MxResolver, SrvRecord, SrvResolution};
pub use orchestrator::detect;
pub use types::{
    AuthKind, DetectError, Detected, DetectedJmap, DetectedMailSettings, DetectedServer, Domain,
    EmailParts, SocketKind, Source, SourceKind,
};
use url::Url;

/// Tuning for one detection run. [`Default`] is the production configuration, only
/// tests and the dev harness construct anything else.
#[derive(Debug, Clone)]
pub struct DetectConfig {
    /// Per-HTTP-request budget (connect + full response). Kept short: detection races
    /// several endpoints that mostly don't exist, and a user is watching.
    pub http_timeout: Duration,
    /// Hard deadline for the whole run, all strategies included.
    pub overall_deadline: Duration,
    /// Redirect hops followed per request before giving up (mirrors the engine's JMAP
    /// session limit).
    pub max_redirects: usize,
    /// Largest response body read before a candidate is discarded.
    pub max_body_bytes: usize,
    /// Budget for one host-side MX resolution.
    pub dns_timeout: Duration,
    /// Dev-harness escape hatch: rebases the JMAP well-known probe onto a local server
    /// (e.g. `http://127.0.0.1:28080` for Stalwart) that the typed domain can't reach,
    /// and waives the probe's HTTPS requirement. Only the bindings' debug/dev-harness
    /// builds ever set it; production code paths cannot.
    pub well_known_base_override: Option<Url>,
}

impl Default for DetectConfig {
    fn default() -> Self {
        Self {
            http_timeout: Duration::from_secs(4),
            overall_deadline: Duration::from_secs(10),
            max_redirects: 5,
            max_body_bytes: 256 * 1024,
            dns_timeout: Duration::from_secs(3),
            well_known_base_override: None,
        }
    }
}
