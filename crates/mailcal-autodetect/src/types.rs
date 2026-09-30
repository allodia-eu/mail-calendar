//! The data model of a detection run: the parsed email address going in, and the
//! discovered server settings (or the reasons there are none) coming out.

use std::fmt;

use crate::hostname::valid_hostname;

/// A validated, normalised DNS domain: lowercased and IDNA/punycode-encoded, so it is
/// safe to embed in lookup URLs and to compare with MX-derived domains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domain(String);

impl Domain {
    /// Parses and normalises `s` (lowercasing, punycoding a Unicode name). Returns
    /// `None` for anything that is not a plain DNS hostname; IP literals included,
    /// since none of the lookup strategies is meaningful for an IP email domain.
    pub fn parse(s: &str) -> Option<Self> {
        let host = url::Host::parse(s.trim()).ok()?;
        let url::Host::Domain(normalized) = host else {
            return None;
        };
        valid_hostname(&normalized).then_some(Self(normalized))
    }

    /// The normalised domain as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An email address split for detection: URL building needs the [`Domain`], placeholder
/// substitution needs all three forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailParts {
    /// The full address as typed (whitespace-trimmed); `%EMAILADDRESS%`.
    pub full: String,
    /// The part before the `@`; `%EMAILLOCALPART%`.
    pub local: String,
    /// The normalised domain after the `@`; `%EMAILDOMAIN%` and every lookup URL.
    pub domain: Domain,
}

impl EmailParts {
    /// Splits `email` at its last `@`. Returns `None` when either side is empty or
    /// whitespace-bearing, or when the domain is not a valid hostname. Deliberately
    /// lenient about the local part beyond that: the servers are the authority on
    /// what a valid mailbox is; detection only needs a usable domain.
    pub fn parse(email: &str) -> Option<Self> {
        let full = email.trim();
        let (local, domain) = full.rsplit_once('@')?;
        if local.is_empty() || local.chars().any(char::is_whitespace) {
            return None;
        }
        let domain = Domain::parse(domain)?;
        Some(Self {
            full: full.to_owned(),
            local: local.to_owned(),
            domain,
        })
    }
}

/// How a detected server secures its connection. Plaintext is unrepresentable by
/// design: a config offering neither TLS nor STARTTLS is a parse error, mirroring
/// Thunderbird's autoconfig parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketKind {
    /// Implicit TLS from the first byte (autoconfig `socketType` "SSL").
    Tls,
    /// Plaintext upgraded in-protocol (autoconfig `socketType` "STARTTLS").
    StartTls,
}

/// How the user authenticates, in the config's stated preference order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    /// A password over the (TLS-protected) connection; autoconfig `password-cleartext`.
    PasswordCleartext,
    /// A challenge-response password scheme (CRAM-MD5 etc.); `password-encrypted`.
    PasswordEncrypted,
    /// OAuth 2.0 bearer authorisation; `OAuth2`.
    OAuth2,
}

/// One server block from a parsed autoconfig document, placeholders already
/// substituted and every field validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedServer {
    /// Server hostname (or IP literal).
    pub hostname: String,
    /// Server port (1–65535; a `0` in the document is a parse error).
    pub port: u16,
    /// Connection security. Never plaintext.
    pub socket: SocketKind,
    /// Accepted authentication schemes in preference order. Never empty.
    pub auth: Vec<AuthKind>,
    /// The login name, `%…%` placeholders substituted (usually the full address).
    pub username: String,
}

/// Machine-readable provenance of a detection result, which strategy produced it and
/// the exact URL it came from, for debug logs and support diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The strategy that found the settings.
    pub kind: SourceKind,
    /// The URL the winning response was fetched from.
    pub url: String,
}

/// The lookup strategy a result came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// The `https://{domain}/.well-known/jmap` probe on the email's own domain.
    JmapWellKnown,
    /// A `_jmap._tcp` SRV record's target, probed at its `/.well-known/jmap`; the
    /// autodiscovery mechanism providers like Fastmail publish instead of the apex.
    JmapSrv,
    /// The provider's own `autoconfig.{domain}` endpoint.
    Autoconfig,
    /// The provider's `/.well-known/autoconfig/…` endpoint.
    AutoconfigWellKnown,
    /// Thunderbird's ISP database (`autoconfig.thunderbird.net`).
    Ispdb,
    /// The RFC 6186/8314 `_imaps._tcp` + `_submissions._tcp` SRV records (implicit TLS).
    ImapSrv,
    /// The provider's autoconfig endpoint for an MX-derived domain.
    MxAutoconfig,
    /// The ISP database queried for an MX-derived domain.
    MxIspdb,
}

/// IMAP/SMTP settings discovered for the email's provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedMailSettings {
    /// Incoming (IMAP) candidates in preference order. Never empty.
    pub incoming: Vec<DetectedServer>,
    /// Outgoing (SMTP) candidates in preference order. Never empty for an autoconfig
    /// document (the parser requires one); **may be empty** for an SRV-derived config
    /// when the domain publishes no implicit-TLS submission record; send stays
    /// unconfigured rather than blocking mail-read.
    pub outgoing: Vec<DetectedServer>,
    /// Whether every step that produced this config was tamper-resistant: every fetch
    /// hop was HTTPS (CA-validated TLS). DNS-derived results (MX/SRV) are trusted on that
    /// TLS alone; DNSSEC is not required. Untrusted settings (a non-HTTPS hop) must be
    /// shown to the user for explicit approval before any credential is sent to them.
    pub is_trusted: bool,
    /// Which strategy and URL produced the config.
    pub source: Source,
    /// The OAuth issuer the provider's **own** autoconfig named (`<oAuth2><issuer>`), as an
    /// HTTPS URL, or `None`.
    ///
    /// Only the endpoints an *issuer* publishes about itself are ever used, never the
    /// `authURL`/`tokenURL`/`clientID` a document writes beside the issuer, and only a
    /// provider describing itself over HTTPS may name one at all: the ISPDB's block is
    /// dropped (`docs/account-autodetect.md` rule 7). `None` is the ordinary case, and the
    /// setup path then looks for an issuer at the provider's own well-known locations
    /// instead.
    pub oauth_issuer: Option<String>,
    /// A CalDAV endpoint discovered for this account, if any: a follow-on RFC 6764
    /// probe (autoconfig/ISPDB describe mail only) found the account's email domain or
    /// its provider's registrable domain advertising `.well-known/caldav` over HTTPS.
    /// `None` means none was found (the user can still add one by hand). Calendar sync
    /// reuses the IMAP credentials at connect; the engine does the authenticated
    /// collection discovery: this is only the unauthenticated "is there one, and where".
    pub caldav_url: Option<String>,
}

/// A JMAP server discovered for the email's domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedJmap {
    /// The base URL to hand the JMAP account setup: `https://{domain}` for an apex
    /// well-known hit, or `https://{srv-target}[:{port}]` for an SRV-discovered endpoint
    /// (e.g. `https://api.fastmail.com`). The engine re-resolves `/.well-known/jmap` from
    /// there at connect time, so a possibly-ephemeral redirect target is never baked in.
    pub base_url: String,
    /// Whether the hit was tamper-resistant: every probe hop was HTTPS (CA-validated TLS).
    /// An SRV-discovered endpoint is trusted on that TLS alone; DNSSEC is not required;
    /// since the resolved host is pinned and re-validated on every connect. Untrusted hits
    /// (a non-HTTPS hop) must be approved before a credential is sent.
    pub is_trusted: bool,
    /// Which strategy and URL produced the hit.
    pub source: Source,
}

/// The outcome of a detection run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detected {
    /// The domain speaks JMAP: the preferred protocol for this product.
    Jmap(DetectedJmap),
    /// IMAP/SMTP settings were published for the domain.
    Mail(DetectedMailSettings),
    /// No strategy produced usable settings.
    Nothing {
        /// `true` when **every** strategy failed on transport (likely offline), as
        /// opposed to a clean "nobody advertises settings for this domain".
        network_error: bool,
    },
}

/// Why a detection run could not start. Mid-run failures never surface here; every
/// per-strategy miss folds into [`Detected::Nothing`].
#[derive(Debug, thiserror::Error)]
pub enum DetectError {
    /// The email address has no usable domain part.
    #[error("invalid email address")]
    InvalidEmail,
    /// The shared TLS trust store could not be built.
    #[error("tls: {0}")]
    Tls(String),
}

#[cfg(test)]
mod tests {
    use super::{Domain, EmailParts};

    #[test]
    fn domain_normalizes_case_and_unicode() {
        assert_eq!(
            Domain::parse("Example.COM").unwrap().as_str(),
            "example.com"
        );
        assert_eq!(
            Domain::parse("münchen.de").unwrap().as_str(),
            "xn--mnchen-3ya.de"
        );
        assert_eq!(
            Domain::parse(" test.local ").unwrap().as_str(),
            "test.local"
        );
    }

    #[test]
    fn domain_rejects_ips_and_garbage() {
        for bad in [
            "",
            "192.168.0.1",
            "[::1]",
            "exa mple.com",
            "under_score.example.com",
        ] {
            assert!(Domain::parse(bad).is_none(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn email_splits_at_last_at_sign() {
        let parts = EmailParts::parse("  alice@Test.Local ").unwrap();
        assert_eq!(parts.full, "alice@Test.Local");
        assert_eq!(parts.local, "alice");
        assert_eq!(parts.domain.as_str(), "test.local");

        let odd = EmailParts::parse("weird@local@example.com").unwrap();
        assert_eq!(odd.local, "weird@local");
        assert_eq!(odd.domain.as_str(), "example.com");
    }

    #[test]
    fn email_keeps_plus_addressing_and_case_in_local_part() {
        let parts = EmailParts::parse("Alice+tag@example.com").unwrap();
        assert_eq!(parts.local, "Alice+tag");
        assert_eq!(parts.full, "Alice+tag@example.com");
    }

    #[test]
    fn email_rejects_missing_or_unusable_parts() {
        for bad in [
            "",
            "no-at-sign",
            "@example.com",
            "alice@",
            "alice@bad domain.com",
            "a b@example.com",
            "alice@192.168.0.1",
        ] {
            assert!(EmailParts::parse(bad).is_none(), "{bad:?} should not parse");
        }
    }
}
