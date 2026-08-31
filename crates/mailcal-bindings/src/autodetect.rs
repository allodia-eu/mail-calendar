//! Automatic server-settings detection over the FFI: the email-first setup step.
//!
//! The host hands over the typed email address (and optionally its platform DNS
//! resolver, the [`MxResolver`] port) and gets back a routed, prefilled
//! [`SetupRecommendation`]: the shared `mailcal-autodetect` engine does the JMAP
//! well-known probe, Mozilla autoconfig, ISPDB, and MX-fallback lookups. Mirrors the
//! password/JMAP setup types in `setup.rs`.

use std::sync::Arc;

use crate::{MailcalApp, setup::ConnectionSecurity};

/// One MX record from the host's resolver, as resolved by the platform DNS API.
#[derive(uniffi::Record)]
pub struct MxRecord {
    /// The record's preference value; lower is more preferred.
    pub preference: u16,
    /// The mail-exchange hostname the record points at.
    pub exchange: String,
}

/// A completed MX lookup from the host resolver.
#[derive(uniffi::Record)]
pub struct MxResolution {
    /// The answer's MX records (any order; the core sorts by preference).
    pub records: Vec<MxRecord>,
    /// Whether the resolver reported the answer DNSSEC-authenticated (the AD header
    /// flag). Pass `false` when the platform API can't tell: the value is not used in
    /// any trust decision today (MX-derived results are trusted on the HTTPS fetch), only
    /// surfaced for a future opt-in "require DNSSEC" setting.
    pub authentic_data: bool,
}

/// One SRV record from the host's resolver (RFC 2782).
#[derive(uniffi::Record)]
pub struct SrvRecord {
    /// Priority; lower is preferred (tried first).
    pub priority: u16,
    /// Weight among equal priorities (not load-bearing for one-shot discovery).
    pub weight: u16,
    /// The TCP port the service listens on.
    pub port: u16,
    /// The target hostname; a single `.` means "the service is explicitly not offered".
    pub target: String,
}

/// A completed SRV lookup from the host resolver.
#[derive(uniffi::Record)]
pub struct SrvResolution {
    /// The answer's SRV records (any order; the core sorts by priority).
    pub records: Vec<SrvRecord>,
    /// Whether the resolver reported the answer DNSSEC-authenticated (the AD header
    /// flag). Pass `false` when the platform API can't tell: the value is not used in
    /// any trust decision today (an SRV-discovered endpoint is trusted on CA-validated
    /// TLS), only surfaced for a future opt-in "require DNSSEC" setting.
    pub authentic_data: bool,
}

/// A host DNS lookup failure, thrown by the platform [`MxResolver`] implementation.
/// Treated as "no MX records"; detection moves on, it never aborts the run.
#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum DnsError {
    /// The lookup failed (no network, NXDOMAIN, timeout, …): the message is for
    /// debug logs only.
    #[error("dns: {0}")]
    Lookup(String),
}

/// The host-DNS port: each client resolves records with its **native** DNS API so the
/// device's real DNS settings (VPN, private DNS) are respected: the core deliberately
/// ships no DNS resolver. Resolves both MX (the MX fallback) and SRV (JMAP and IMAP/SMTP
/// autodiscovery). Called on a worker thread; the implementation may block. Optional; a
/// host that passes no resolver skips the MX fallback and the SRV strategies.
#[uniffi::export(callback_interface)]
pub trait MxResolver: Send + Sync {
    /// Resolves the MX records for `domain` (an ASCII/punycode DNS name).
    ///
    /// # Errors
    ///
    /// Throws [`DnsError`] when the lookup fails outright; return an empty
    /// [`MxResolution::records`] for a clean "no MX records" answer.
    fn resolve_mx(&self, domain: String) -> Result<MxResolution, DnsError>;

    /// Resolves the SRV records for the owner name `name` (e.g.
    /// `_jmap._tcp.example.com`, `_imaps._tcp.example.com`); how a provider like Fastmail
    /// advertises a JMAP or IMAP/SMTP endpoint that isn't on the apex domain.
    ///
    /// # Errors
    ///
    /// Throws [`DnsError`] when the lookup fails outright; return an empty
    /// [`SrvResolution::records`] for a clean "no SRV records" answer.
    fn resolve_srv(&self, name: String) -> Result<SrvResolution, DnsError>;
}

/// One detected server, formatted for the result card a host shows before the user
/// commits ("IMAP · imap.example.com · 993 · SSL/TLS · alice@example.com").
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DetectedServerRow {
    /// The protocol label: `"IMAP"` or `"SMTP"`.
    pub protocol: String,
    /// The server hostname.
    pub hostname: String,
    /// The server port.
    pub port: u16,
    /// The connection-security label for the card: `"SSL/TLS"` (implicit TLS) or
    /// `"STARTTLS"`.
    pub security: String,
    /// The login username the config prescribes (usually the full email address).
    pub username: String,
}

/// Why detection routed to manual setup, so the host can show a localised reason line.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissReason {
    /// The typed text has no usable email domain.
    InvalidEmail,
    /// Every lookup came back clean-empty: nobody advertises settings for the domain.
    NothingFound,
    /// Every lookup failed on transport; likely offline; worth a retry.
    NetworkError,
    /// A config exists but only offers OAuth at a provider this app has no OAuth
    /// integration for.
    OauthOnlyProvider,
}

/// Where the email-first setup step routes, with everything the target form needs
/// prefilled. When `is_trusted` is `false` the host MUST show the untrusted-settings
/// warning and require explicit approval before Connect (docs/account-autodetect.md).
///
/// The `Imap` variant is much larger than `Manual`/`Microsoft`, but this value is
/// built once per detection and handed straight across the FFI; boxing a variant's
/// payload isn't expressible as a UniFFI enum, and the size is irrelevant here.
#[allow(clippy::large_enum_variant)]
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum SetupRecommendation {
    /// The domain speaks JMAP; route to the JMAP form (password/token entry only).
    Jmap {
        /// The typed email address, prefilled.
        email: String,
        /// The JMAP server URL for [`crate::JmapSetup::server_url`].
        server_url: String,
        /// Whether every probe hop was HTTPS (see the enum docs for the `false` rule).
        is_trusted: bool,
        /// Provenance: the strategy and URL that produced this, for diagnostics.
        source: String,
    },
    /// IMAP/SMTP settings were published; route to the password form, prefilled.
    Imap {
        /// The typed email address, prefilled.
        email: String,
        /// The IMAP server for [`crate::AccountSetup::imap_host`] (`host`, or
        /// `host:port` when the port isn't 993).
        imap_host: String,
        /// The SMTP server for [`crate::AccountSetup::smtp_host`] (`host`, or
        /// `host:port` when the port isn't standard); `None` when the provider publishes no
        /// SMTP server this engine can use; mail-send stays unconfigured.
        smtp_host: Option<String>,
        /// How the incoming (IMAP) connection is secured: the host passes this straight
        /// back as [`crate::AccountSetup::imap_security`] on connect.
        imap_security: ConnectionSecurity,
        /// How the outgoing (SMTP) connection is secured; passed back as
        /// [`crate::AccountSetup::smtp_security`]; meaningful only when `smtp_host` is `Some`.
        smtp_security: ConnectionSecurity,
        /// The incoming server, formatted for the result card.
        incoming: DetectedServerRow,
        /// The outgoing server for the result card; `None` exactly when `smtp_host` is.
        outgoing: Option<DetectedServerRow>,
        /// A CalDAV endpoint discovered for the account (RFC 6764), or `None` when none
        /// was found. When present, the host shows the calendar toggle **pre-checked**
        /// (opt-out) and passes this as [`crate::AccountSetup::caldav_base_url`] on
        /// connect, reusing the IMAP credentials, when `None`, it offers an opt-in manual
        /// CalDAV field. The engine does the real authenticated discovery at connect.
        caldav_url: Option<String>,
        /// Whether every step that produced this config was tamper-resistant: every hop
        /// was HTTPS (CA-validated TLS). DNS-derived results (MX/SRV) are trusted on that
        /// TLS alone; DNSSEC is not required.
        is_trusted: bool,
        /// Provenance: the strategy and URL that produced this, for diagnostics.
        source: String,
    },
    /// The domain is Microsoft-hosted; suggest the Microsoft 365 sign-in path.
    Microsoft {
        /// The typed email address, prefilled.
        email: String,
    },
    /// The domain is Google-hosted (consumer Gmail or a Google Workspace domain); suggest the
    /// native Google sign-in path (Gmail + Google Calendar). The host gates this behind Early
    /// Access: it shows the Early-Access notice and requires the user to confirm they signed up
    /// before starting the OAuth flow.
    Google {
        /// The typed email address, prefilled.
        email: String,
    },
    /// Nothing usable was found; route to today's manual tabs with a reason line.
    Manual {
        /// Why detection came up empty.
        reason: MissReason,
    },
}

#[uniffi::export]
impl MailcalApp {
    /// Looks up mail-server settings for `email`: a JMAP well-known probe, Mozilla
    /// autoconfig, the ISPDB, and (when the host passes its resolver) the MX fallback,
    /// raced in that priority order. Blocking, bounded to ~10 s worst case; call it
    /// off the main thread exactly like `add_account`. Never throws: every miss folds
    /// into [`SetupRecommendation::Manual`] so the setup form always has a route.
    pub fn detect_account_settings(
        &self,
        email: String,
        mx_resolver: Option<Box<dyn MxResolver>>,
    ) -> SetupRecommendation {
        if self.showcase {
            return showcase_recommendation(&email);
        }
        let resolver = mx_resolver.map(|resolver| {
            Arc::new(CallbackMxResolver(resolver)) as Arc<dyn mailcal_autodetect::MxResolver>
        });
        let config = detect_config();
        let result = self
            .runtime
            .block_on(mailcal_autodetect::detect(&email, resolver, &config));
        to_recommendation(&email, result)
    }
}

/// The domain whose canned detection is the **happy path**: everything published over HTTPS,
/// with a calendar found beside the mailbox. The showcase's work account lives here, so the
/// documentation's setup walkthrough and its mailbox screenshots tell one story.
const SHOWCASE_TRUSTED_DOMAIN: &str = "northwind.example";

/// The domain whose canned detection comes back **untrusted**; settings that were only
/// reachable over a plain-HTTP hop. This is the screen that matters most in the setup
/// documentation: the one place a user is asked to approve something before a password is sent
/// (docs/account-autodetect.md), and the one a real provider gives us no reliable way to stage.
const SHOWCASE_UNTRUSTED_DOMAIN: &str = "oldschool.example";

/// Detection's answer in a showcase (screenshot) build: scripted, instant, and offline.
///
/// Three outcomes, because the account-setup documentation has three screens to show; the
/// settings were found and are trustworthy, they were found over an insecure hop and need
/// approval, and nothing was found so the manual form takes over. Every other address falls to
/// the last one, which is what makes the personal-domain example in the guide land on the manual
/// route without a special case.
///
/// Both domains are `.example` (RFC 2606), so even if this were ever reached outside a showcase
/// build it could not name a host that resolves. Adding the Microsoft or Google route here is a
/// new arm and nothing else, once a guide needs to picture one.
fn showcase_recommendation(email: &str) -> SetupRecommendation {
    let Some((_, domain)) = email.rsplit_once('@') else {
        return SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail,
        };
    };
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail,
        };
    }
    let trusted = domain == SHOWCASE_TRUSTED_DOMAIN;
    if !trusted && domain != SHOWCASE_UNTRUSTED_DOMAIN {
        return SetupRecommendation::Manual {
            reason: MissReason::NothingFound,
        };
    }
    let imap_host = format!("imap.{domain}");
    let smtp_host = format!("smtp.{domain}");
    SetupRecommendation::Imap {
        email: email.to_owned(),
        imap_host: imap_host.clone(),
        smtp_host: Some(smtp_host.clone()),
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        incoming: DetectedServerRow {
            protocol: "IMAP".to_owned(),
            hostname: imap_host,
            port: 993,
            security: "SSL/TLS".to_owned(),
            username: email.to_owned(),
        },
        outgoing: Some(DetectedServerRow {
            protocol: "SMTP".to_owned(),
            hostname: smtp_host,
            port: 465,
            security: "SSL/TLS".to_owned(),
            username: email.to_owned(),
        }),
        // Only the trusted domain publishes a calendar, so the guide can show the pre-checked
        // opt-out toggle on one screen and its absence on the other.
        caldav_url: trusted.then(|| format!("https://dav.{domain}/")),
        is_trusted: trusted,
        source: if trusted {
            format!("autoconfig (https://autoconfig.{domain}/mail/config-v1.1.xml)")
        } else {
            format!("autoconfig (http://autoconfig.{domain}/mail/config-v1.1.xml)")
        },
    }
}

/// Adapts the host's FFI [`MxResolver`] to the detection crate's DNS port, mapping the
/// records across and a [`DnsError`] to the crate's lookup error.
struct CallbackMxResolver(Box<dyn MxResolver>);

impl mailcal_autodetect::MxResolver for CallbackMxResolver {
    fn resolve_mx(
        &self,
        domain: &str,
    ) -> Result<mailcal_autodetect::MxResolution, mailcal_autodetect::MxError> {
        let resolution = self
            .0
            .resolve_mx(domain.to_owned())
            .map_err(|DnsError::Lookup(message)| mailcal_autodetect::MxError::Lookup(message))?;
        Ok(mailcal_autodetect::MxResolution {
            records: resolution
                .records
                .into_iter()
                .map(|record| mailcal_autodetect::MxRecord {
                    preference: record.preference,
                    exchange: record.exchange,
                })
                .collect(),
            authentic_data: resolution.authentic_data,
        })
    }

    fn resolve_srv(
        &self,
        name: &str,
    ) -> Result<mailcal_autodetect::SrvResolution, mailcal_autodetect::MxError> {
        let resolution = self
            .0
            .resolve_srv(name.to_owned())
            .map_err(|DnsError::Lookup(message)| mailcal_autodetect::MxError::Lookup(message))?;
        Ok(mailcal_autodetect::SrvResolution {
            records: resolution
                .records
                .into_iter()
                .map(|record| mailcal_autodetect::SrvRecord {
                    priority: record.priority,
                    weight: record.weight,
                    port: record.port,
                    target: record.target,
                })
                .collect(),
            authentic_data: resolution.authentic_data,
        })
    }
}

/// The production detection tuning, plus the debug/dev-harness well-known-base override
/// (`MAILCAL_AUTODETECT_WELL_KNOWN_BASE`) so `alice@test.local` reaches the local
/// Stalwart. The override is compiled out of a release build without `dev-harness`.
fn detect_config() -> mailcal_autodetect::DetectConfig {
    #[allow(unused_mut)]
    let mut config = mailcal_autodetect::DetectConfig::default();
    #[cfg(any(debug_assertions, feature = "dev-harness"))]
    if let Ok(base) = std::env::var("MAILCAL_AUTODETECT_WELL_KNOWN_BASE") {
        match url::Url::parse(&base) {
            Ok(url) => config.well_known_base_override = Some(url),
            Err(err) => log::warn!("ignoring invalid MAILCAL_AUTODETECT_WELL_KNOWN_BASE: {err}"),
        }
    }
    config
}

/// Maps a detection result for `email` onto the FFI recommendation. A pre-flight error
/// (invalid email, TLS build failure) folds into a manual route so the form always has
/// somewhere to go.
fn to_recommendation(
    email: &str,
    result: Result<mailcal_autodetect::Detected, mailcal_autodetect::DetectError>,
) -> SetupRecommendation {
    match result {
        Ok(detected) => convert(mailcal_account::recommend(
            email,
            detected,
            mailcal_account::OauthRoutes::of_this_build(),
        )),
        Err(mailcal_autodetect::DetectError::InvalidEmail) => SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail,
        },
        Err(mailcal_autodetect::DetectError::Tls(_)) => SetupRecommendation::Manual {
            reason: MissReason::NetworkError,
        },
    }
}

/// Converts the account layer's recommendation into its FFI mirror.
fn convert(recommendation: mailcal_account::SetupRecommendation) -> SetupRecommendation {
    use mailcal_account::SetupRecommendation as R;
    match recommendation {
        R::Jmap {
            email,
            server_url,
            is_trusted,
            source,
        } => SetupRecommendation::Jmap {
            email,
            server_url,
            is_trusted,
            source,
        },
        R::Imap {
            email,
            imap_host,
            smtp_host,
            imap_security,
            smtp_security,
            incoming,
            outgoing,
            caldav_url,
            is_trusted,
            source,
        } => SetupRecommendation::Imap {
            email,
            imap_host,
            smtp_host,
            imap_security: imap_security.into(),
            smtp_security: smtp_security.into(),
            incoming: convert_row(incoming),
            outgoing: outgoing.map(convert_row),
            caldav_url,
            is_trusted,
            source,
        },
        R::Microsoft { email } => SetupRecommendation::Microsoft { email },
        R::Google { email } => SetupRecommendation::Google { email },
        R::Manual { reason } => SetupRecommendation::Manual {
            reason: convert_reason(reason),
        },
    }
}

/// Converts a server summary into its FFI row.
fn convert_row(summary: mailcal_account::ServerSummary) -> DetectedServerRow {
    DetectedServerRow {
        protocol: summary.protocol,
        hostname: summary.hostname,
        port: summary.port,
        security: summary.security,
        username: summary.username,
    }
}

/// Converts a miss reason into its FFI mirror.
fn convert_reason(reason: mailcal_account::MissReason) -> MissReason {
    use mailcal_account::MissReason as R;
    match reason {
        R::InvalidEmail => MissReason::InvalidEmail,
        R::NothingFound => MissReason::NothingFound,
        R::NetworkError => MissReason::NetworkError,
        R::OauthOnlyProvider => MissReason::OauthOnlyProvider,
    }
}

#[cfg(test)]
#[path = "autodetect_tests.rs"]
mod autodetect_tests;
