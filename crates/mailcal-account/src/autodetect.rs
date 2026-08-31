//! Maps a protocol-neutral [`Detected`] result onto a concrete setup route the clients
//! can prefill; encoding what *this* product can actually connect.
//!
//! The detection crate reports what a domain advertises; this layer decides what to do
//! with it, using facts that live in the account layer, not the detector: our engine
//! speaks implicit TLS and STARTTLS (but no plaintext), it has native OAuth integrations
//! for **Microsoft** and **Google** (so those route to a browser sign-in rather than IMAP),
//! and the setup builders take a bare host with an assumed standard port. Keeping the
//! decision here means `mailcal-autodetect` stays product-neutral.

use mailcal_autodetect::{AuthKind, Detected, DetectedServer, SocketKind};

use crate::ConnectionSecurity;

/// The standard implicit-TLS IMAP port: a detected server on it needs no explicit port.
const DEFAULT_IMAP_TLS_PORT: u16 = 993;
/// The standard STARTTLS IMAP port.
const DEFAULT_IMAP_STARTTLS_PORT: u16 = 143;
/// The standard implicit-TLS SMTP submission port.
const DEFAULT_SMTP_TLS_PORT: u16 = 465;
/// The standard STARTTLS SMTP submission port.
const DEFAULT_SMTP_STARTTLS_PORT: u16 = 587;

/// Where the email-first setup step should route, with everything the target form needs
/// prefilled.
///
/// The `Imap` variant is much larger than `Microsoft`/`Manual`, but a recommendation is
/// built once per detection and handed straight to the client (via the FFI mirror, where
/// boxing isn't expressible), so the size disparity has no practical cost.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupRecommendation {
    /// The domain speaks JMAP; route to the JMAP form.
    Jmap {
        /// The typed email address.
        email: String,
        /// The JMAP base URL to prefill.
        server_url: String,
        /// Whether the settings were obtained tamper-resistantly (see [`Detected`]).
        is_trusted: bool,
        /// Provenance, for diagnostics.
        source: String,
    },
    /// IMAP/SMTP settings were found; route to the password form.
    Imap {
        /// The typed email address.
        email: String,
        /// The incoming server for the setup builder (`host`, or `host:port` when the
        /// port is non-standard).
        imap_host: String,
        /// The outgoing server for the builder, or `None` when the provider publishes no
        /// SMTP this engine can use.
        smtp_host: Option<String>,
        /// How the incoming (IMAP) connection is secured: the client passes this straight
        /// back on connect so the engine dials implicit TLS or STARTTLS as detected.
        imap_security: ConnectionSecurity,
        /// How the outgoing (SMTP) connection is secured; meaningful only when `smtp_host`
        /// is `Some` (implicit TLS otherwise, unused).
        smtp_security: ConnectionSecurity,
        /// The incoming server, summarised for the confirmation card.
        incoming: ServerSummary,
        /// The outgoing server for the card; `None` exactly when `smtp_host` is.
        outgoing: Option<ServerSummary>,
        /// A CalDAV endpoint discovered for the account (RFC 6764), or `None` when none
        /// was found. When present, the client offers calendar sync **pre-selected**
        /// (opt-out), reusing the IMAP credentials, when `None`, it offers an opt-in
        /// manual CalDAV field. This is a discovery hint: the engine does the real
        /// authenticated collection discovery at connect.
        caldav_url: Option<String>,
        /// Whether the settings were obtained tamper-resistantly.
        is_trusted: bool,
        /// Provenance, for diagnostics.
        source: String,
    },
    /// The domain is Microsoft-hosted; steer to the Microsoft 365 sign-in.
    Microsoft {
        /// The typed email address.
        email: String,
    },
    /// The domain is Google-hosted (consumer Gmail or a Google Workspace domain); steer to
    /// the native Google sign-in (Gmail + Google Calendar over Google's own APIs), which the
    /// client gates behind Early Access.
    Google {
        /// The typed email address.
        email: String,
    },
    /// Nothing usable; fall back to manual setup with a reason.
    Manual {
        /// Why detection did not produce a usable route.
        reason: MissReason,
    },
}

/// A detected server rendered for the confirmation card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSummary {
    /// The protocol label (`"IMAP"` / `"SMTP"`).
    pub protocol: String,
    /// The hostname.
    pub hostname: String,
    /// The port.
    pub port: u16,
    /// The connection-security label for the card: `"SSL/TLS"` (implicit TLS) or
    /// `"STARTTLS"`.
    pub security: String,
    /// The login username the config prescribes.
    pub username: String,
}

/// Why detection routed to manual setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissReason {
    /// The typed text had no usable email domain.
    InvalidEmail,
    /// Every lookup came back clean-empty.
    NothingFound,
    /// Every lookup failed on transport; likely offline.
    NetworkError,
    /// A config exists but offers only OAuth at a provider we have no integration for.
    OauthOnlyProvider,
}

/// Which browser sign-in routes this build can actually offer.
///
/// A route needs an OAuth client registration, and a build carries one only if it was given one
/// ([`mailcal_oauth::credentials`]). Routing to a sign-in the build cannot start would put the
/// user in front of a button that fails at the provider, so detection is told what exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OauthRoutes {
    /// Whether the native Google (Gmail + Google Calendar) sign-in can be offered.
    pub google: bool,
    /// Whether the Microsoft 365 sign-in can be offered.
    pub microsoft: bool,
}

impl OauthRoutes {
    /// The routes this build's injected registrations allow.
    #[must_use]
    pub fn of_this_build() -> Self {
        Self {
            google: mailcal_oauth::credentials::google().is_some(),
            microsoft: mailcal_oauth::credentials::microsoft_client_id().is_some(),
        }
    }
}

/// Consumer Google domains that always route to the native Google sign-in, without waiting on
/// detection: the domain is already in the typed address, so no lookup (and no privacy
/// disclosure) is needed. A Google **Workspace** custom domain instead reveals Google via its
/// incoming host ([`is_google_family`]), the same way Microsoft-hosted domains are detected.
const GOOGLE_CONSUMER_DOMAINS: &[&str] = &["gmail.com", "googlemail.com"];

/// Whether the typed address's domain is a consumer Google domain.
fn is_google_consumer_domain(email: &str) -> bool {
    email
        .rsplit('@')
        .next()
        .map(str::to_ascii_lowercase)
        .is_some_and(|domain| GOOGLE_CONSUMER_DOMAINS.contains(&domain.as_str()))
}

/// Turns a detection result for `email` into a routed, prefilled recommendation, offering only
/// the browser sign-ins `routes` says this build carries a registration for.
#[must_use]
pub fn recommend(email: &str, detected: Detected, routes: OauthRoutes) -> SetupRecommendation {
    // A consumer Google address routes to the native Google sign-in regardless of what
    // detection turned up (the domain is already local: no lookup needed, and gmail.com's
    // ISPDB entry is an IMAP app-password we deliberately supersede). A Workspace domain is
    // caught downstream by its Google incoming host.
    if routes.google && is_google_consumer_domain(email) {
        return SetupRecommendation::Google {
            email: email.to_owned(),
        };
    }
    match detected {
        Detected::Jmap(jmap) => SetupRecommendation::Jmap {
            email: email.to_owned(),
            server_url: jmap.base_url,
            is_trusted: jmap.is_trusted,
            source: jmap.source.url,
        },
        Detected::Mail(settings) => recommend_mail(email, &settings, routes),
        Detected::Nothing { network_error } => SetupRecommendation::Manual {
            reason: if network_error {
                MissReason::NetworkError
            } else {
                MissReason::NothingFound
            },
        },
    }
}

/// The mail-settings branch: Microsoft-family first, then a TLS+password incoming, else
/// a reasoned fall-back to manual.
fn recommend_mail(
    email: &str,
    settings: &mailcal_autodetect::DetectedMailSettings,
    routes: OauthRoutes,
) -> SetupRecommendation {
    if settings.incoming.iter().any(is_microsoft_family) {
        // Microsoft retired Basic auth, so ISPDB's listed password methods are stale;
        // the only route that works is the browser OAuth sign-in. A build without the
        // registration to start one has nothing to offer this domain, and the prefilled IMAP
        // form ISPDB would produce is a login that cannot succeed: so say so instead.
        return if routes.microsoft {
            SetupRecommendation::Microsoft {
                email: email.to_owned(),
            }
        } else {
            SetupRecommendation::Manual {
                reason: MissReason::OauthOnlyProvider,
            }
        };
    }

    if routes.google && settings.incoming.iter().any(is_google_family) {
        // A Google Workspace domain (its incoming host is Google's). We have a native
        // Gmail/Calendar integration, so route to the Google browser sign-in rather than the
        // IMAP app-password ISPDB would otherwise prefill. Without the registration, that
        // app-password route is the one below and it still works; unlike Microsoft's.
        return SetupRecommendation::Google {
            email: email.to_owned(),
        };
    }

    let Some(incoming) = settings
        .incoming
        .iter()
        .find(|server| is_connectable(server))
    else {
        // Every detected server is TLS or STARTTLS (plaintext is a parse error), so the
        // only way nothing is connectable is that no server offers a password login: an
        // OAuth-only provider we have no integration for.
        return SetupRecommendation::Manual {
            reason: MissReason::OauthOnlyProvider,
        };
    };
    let outgoing = settings
        .outgoing
        .iter()
        .find(|server| is_connectable(server));

    SetupRecommendation::Imap {
        email: email.to_owned(),
        imap_host: host_field(incoming, imap_default_port(incoming.socket)),
        smtp_host: outgoing.map(|server| host_field(server, smtp_default_port(server.socket))),
        imap_security: security_of(incoming.socket),
        smtp_security: outgoing.map_or(ConnectionSecurity::ImplicitTls, |s| security_of(s.socket)),
        incoming: summary(incoming, "IMAP"),
        outgoing: outgoing.map(|server| summary(server, "SMTP")),
        caldav_url: settings.caldav_url.clone(),
        is_trusted: settings.is_trusted,
        source: settings.source.url.clone(),
    }
}

/// Whether our engine can connect this server: a TLS-secured link (implicit TLS or
/// STARTTLS; both carry credentials only over TLS) with a password login.
fn is_connectable(server: &DetectedServer) -> bool {
    server.auth.iter().copied().any(is_password)
}

/// Whether an auth method is a password scheme (either is a password over the TLS link).
fn is_password(auth: AuthKind) -> bool {
    matches!(
        auth,
        AuthKind::PasswordCleartext | AuthKind::PasswordEncrypted
    )
}

/// Maps a detected socket kind onto the account layer's connection-security setting the
/// client passes back on connect.
const fn security_of(socket: SocketKind) -> ConnectionSecurity {
    match socket {
        SocketKind::Tls => ConnectionSecurity::ImplicitTls,
        SocketKind::StartTls => ConnectionSecurity::StartTls,
    }
}

/// The standard IMAP port for a detected socket kind: a server on it needs no explicit port.
const fn imap_default_port(socket: SocketKind) -> u16 {
    match socket {
        SocketKind::Tls => DEFAULT_IMAP_TLS_PORT,
        SocketKind::StartTls => DEFAULT_IMAP_STARTTLS_PORT,
    }
}

/// The standard SMTP submission port for a detected socket kind.
const fn smtp_default_port(socket: SocketKind) -> u16 {
    match socket {
        SocketKind::Tls => DEFAULT_SMTP_TLS_PORT,
        SocketKind::StartTls => DEFAULT_SMTP_STARTTLS_PORT,
    }
}

/// The confirmation-card security label for a detected socket kind.
const fn security_label(socket: SocketKind) -> &'static str {
    match socket {
        SocketKind::Tls => "SSL/TLS",
        SocketKind::StartTls => "STARTTLS",
    }
}

/// Whether a server hostname belongs to Microsoft 365 / Outlook.
fn is_microsoft_family(server: &DetectedServer) -> bool {
    let host = server.hostname.to_ascii_lowercase();
    host == "outlook.office365.com"
        || host.ends_with(".office365.com")
        || host.ends_with(".outlook.com")
}

/// Whether a server hostname belongs to Google (consumer Gmail or Google Workspace; both use
/// `imap.gmail.com`/`smtp.gmail.com`). Catches a Workspace custom domain whose autoconfig/MX
/// reveals Google's mail hosts.
fn is_google_family(server: &DetectedServer) -> bool {
    let host = server.hostname.to_ascii_lowercase();
    host.ends_with(".gmail.com")
        || host.ends_with(".google.com")
        || host.ends_with(".googlemail.com")
        || host == "gmail.com"
        || host == "googlemail.com"
}

/// The setup-builder host field: a bare host on the standard port, else `host:port`.
fn host_field(server: &DetectedServer, default_port: u16) -> String {
    if server.port == default_port {
        server.hostname.clone()
    } else {
        format!("{}:{}", server.hostname, server.port)
    }
}

/// Summarises a server for the confirmation card.
fn summary(server: &DetectedServer, protocol: &str) -> ServerSummary {
    ServerSummary {
        protocol: protocol.to_owned(),
        hostname: server.hostname.clone(),
        port: server.port,
        security: security_label(server.socket).to_owned(),
        username: server.username.clone(),
    }
}

#[cfg(test)]
#[path = "autodetect_tests.rs"]
mod autodetect_tests;
