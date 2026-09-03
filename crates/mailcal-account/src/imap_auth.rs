//! What a mail account should be asked for at setup: sign in with the provider, or type a
//! password.
//!
//! Account setup is where people give up, so this decides as much as it can from evidence
//! rather than asking. Three answers, in the order they are preferred:
//!
//! 1. **Sign in.** The server takes an OAuth token *and* an authorization server was found that
//!    this install can register with (RFC 8414 → RFC 7591), or that this build already holds a
//!    registration for. The user signs in at their provider and types nothing here.
//! 2. **Sign-in exists, but not for us.** The server takes a token and no usable authorization
//!    server was found: the provider admits only applications it registered in advance. Saying so
//!    is worth a line of copy, because "use a password instead" without the reason reads like the
//!    app is broken.
//! 3. **Password.** The server advertises no OAuth mechanism at all. Today's form, unchanged.
//!
//! # The order of evidence matters
//!
//! The server is asked **first** ([`provider_imap::probe_imap_auth`]), before any of the
//! OAuth discovery runs. A domain can publish an authorization server for its web sessions
//! and take only a password on IMAP, and offering sign-in on that evidence produces a token
//! the mailbox refuses. The capability line is what the account will actually be judged by.
//!
//! # Where an issuer may come from
//!
//! Two channels, both the provider describing itself, never a third party
//! ([`docs/account-autodetect.md`](../../../docs/account-autodetect.md) rule 7):
//!
//! - the `<oAuth2><issuer>` of the provider's **own** HTTPS autoconfig, which detection already
//!   fetched and which the parser carries through;
//! - failing that, an RFC 8414 probe of the **email domain** and the **IMAP server's registrable
//!   domain**, at their own well-known locations.
//!
//! A JMAP account gets its issuer from an RFC 9728 `401` challenge instead. IMAP has no such
//! HTTP surface to be challenged on, which is why this exists at all.

use mailcal_oauth::AuthServerMetadata;
use provider_imap::{AuthOffer, ImapSecurity, probe_imap_auth, probe_smtp_auth};

use crate::{ConnectionSecurity, tls::account_tls};

/// What setup should ask this account for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImapAuth {
    /// Offer "sign in with your provider" as the primary action.
    SignIn {
        /// The authorization server that will be used, for the diagnostic log and for the
        /// host to show which server it is about to send the user to.
        issuer: String,
        /// The provider's name for the button, when this build holds a **static**
        /// registration that names one. `None` for a server discovered from the standards,
        /// which tells us no name to print, and where the button says so generically.
        provider_label: Option<String>,
        /// Whether the server also accepts a password, so the client can offer "use a
        /// password instead" beside the button. `false` on a server that has switched
        /// password auth off, where that link would be a dead end.
        password_also_works: bool,
    },
    /// The server takes an OAuth token, but only from an application it registered in
    /// advance, and this build holds no registration for it.
    RegistrationNeeded {
        /// Whether a password still works. Almost always `true`: a provider in this position
        /// generally issues app-specific passwords precisely because not every client can do
        /// its OAuth.
        password_also_works: bool,
    },
    /// The server advertises no OAuth mechanism, or did not answer. Ask for a password, which
    /// is what works everywhere.
    Password,
}

/// Where to look for an authorization server, and what the mail server already said.
#[derive(Debug, Clone)]
pub struct ImapAuthQuery {
    /// The IMAP server to probe: a host, or `host:port`.
    pub imap_host: String,
    /// How the IMAP connection is secured, so the probe reads the capability the session
    /// would actually see.
    pub imap_security: ConnectionSecurity,
    /// The account's email address, whose domain is both an issuer candidate and the `EHLO`
    /// name a submission would introduce itself with.
    pub email: String,
    /// The issuer the provider's own autoconfig named, when detection found one.
    pub autoconfig_issuer: Option<String>,
}

/// Decides what setup should ask for. **Blocking-ish**: makes one TLS connection to the mail
/// server and up to four short HTTPS requests, all bounded by their own clients' timeouts.
///
/// Never fails. Every unanswered question resolves to [`ImapAuth::Password`], which works
/// everywhere; a setup screen that showed an error here would be blocking the user on a
/// question they did not ask.
pub async fn decide_imap_auth(query: &ImapAuthQuery) -> ImapAuth {
    let Some(offer) = probe(query).await else {
        return ImapAuth::Password;
    };
    if !offer.oauth {
        log::info!(
            "imap auth: the server advertises no OAuth mechanism; offering a password (it offers: {})",
            mechanisms(&offer),
        );
        return ImapAuth::Password;
    }
    let password_also_works = offer.password;

    // A registration we already hold beats discovery: it means the provider has told us in
    // advance which client we are, and re-registering would be neither possible nor wanted.
    let host = host_of(&query.imap_host);
    if let Some(provider) =
        mailcal_oauth::provider_for_host(&mailcal_oauth::static_mail_providers(), host)
    {
        log::info!(
            "imap auth: using this build's registration for {} at {}",
            provider.label,
            provider.issuer,
        );
        return ImapAuth::SignIn {
            issuer: provider.issuer.to_owned(),
            provider_label: Some(provider.label.to_owned()),
            password_also_works,
        };
    }

    if let Some(metadata) = imap_issuer(query).await {
        log::info!(
            "imap auth: {} offers open registration; sign-in can be offered",
            metadata.issuer,
        );
        return ImapAuth::SignIn {
            issuer: metadata.issuer,
            provider_label: None,
            password_also_works,
        };
    }
    log::info!(
        "imap auth: the server takes an OAuth token but names no authorization server we can register with"
    );
    ImapAuth::RegistrationNeeded {
        password_also_works,
    }
}

/// Asks the mail server what it accepts, or `None` when it did not answer.
///
/// The **IMAP** answer decides, because that is the session the account is built on; the SMTP
/// probe runs only to say something useful in the log when the two disagree, which is a
/// provider misconfiguration a user would otherwise meet as "sending doesn't work" weeks
/// later.
async fn probe(query: &ImapAuthQuery) -> Option<AuthOffer> {
    let tls = account_tls().ok()?;
    let host = host_of(&query.imap_host);
    let addr = dial_addr(&query.imap_host, query.imap_security);
    let security = engine_security(query.imap_security);
    match probe_imap_auth(&addr, host, security, &tls.connector()).await {
        Ok(offer) => {
            log::info!(
                "imap auth: {host} offers [{}] (oauth: {}, password: {})",
                mechanisms(&offer),
                offer.oauth,
                offer.password,
            );
            Some(offer)
        }
        Err(err) => {
            log::info!("imap auth: {host} did not answer the capability probe; {err}");
            None
        }
    }
}

/// Finds an authorization server for this account, preferring what the provider's own
/// autoconfig named and falling back to the well-known locations of the domains the account
/// actually involves.
///
/// A candidate is only accepted when its metadata advertises a `registration_endpoint`: that
/// is what mints this install's client id, and a server without one has told us it does not
/// admit clients it has not met ([`ImapAuth::RegistrationNeeded`]).
///
/// Returns the **metadata**, not just the issuer, because the caller that goes on to register
/// needs every field of it and re-fetching would be a second round trip that could disagree
/// with the first.
pub async fn imap_issuer(query: &ImapAuthQuery) -> Option<AuthServerMetadata> {
    let http = mailcal_oauth::discovery_client().ok()?;
    let host = host_of(&query.imap_host);
    for issuer in issuer_candidates(query, host) {
        match mailcal_oauth::discover_auth_server(&http, &issuer).await {
            Ok(metadata) if metadata.registration_endpoint.is_some() => return Some(metadata),
            Ok(metadata) => log::info!(
                "imap auth: {} publishes metadata but no registration endpoint",
                metadata.issuer,
            ),
            Err(err) => log::debug!("imap auth: no authorization server at {issuer}; {err}"),
        }
    }
    None
}

/// The issuers to try, in order: the one the provider's own autoconfig named, then the email
/// domain, then the mail server's registrable domain.
///
/// The last is what covers a self-hosted server whose mailboxes are `@example.com` while the
/// server answers as `mail.example.com`, and a hosted provider whose customer domain
/// publishes nothing of its own.
fn issuer_candidates(query: &ImapAuthQuery, host: &str) -> Vec<String> {
    let mut candidates: Vec<String> = query.autoconfig_issuer.iter().cloned().collect();
    let mut add = |domain: &str| {
        let candidate = format!("https://{domain}");
        if !domain.is_empty() && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };
    if let Some((_, domain)) = query.email.trim().rsplit_once('@') {
        add(&domain.trim().to_ascii_lowercase());
    }
    add(&registrable_domain(host));
    add(host);
    candidates
}

/// The registrable domain of `host`: its last two labels.
///
/// Deliberately not a Public Suffix List lookup. This produces a *candidate to probe*, and a
/// wrong guess costs one HTTPS request that finds nothing; the PSL matters where a wrong
/// answer would be acted on, which is the MX-derivation in `mailcal-autodetect`, and that is
/// where it lives.
fn registrable_domain(host: &str) -> String {
    let labels: Vec<&str> = host.split('.').filter(|label| !label.is_empty()).collect();
    if labels.len() < 2 {
        return host.to_owned();
    }
    labels[labels.len() - 2..].join(".")
}

/// The host part of a `host` or `host:port` server field.
fn host_of(server: &str) -> &str {
    let server = server.trim();
    match server.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => server,
    }
}

/// The `host:port` to dial, applying the standard port for the chosen security when the field
/// carries none: the same defaulting the setup builder does, so a probe and the connect that
/// follows it reach the same server.
fn dial_addr(server: &str, security: ConnectionSecurity) -> String {
    let server = server.trim();
    if host_of(server) == server {
        let port = match security {
            ConnectionSecurity::ImplicitTls => 993,
            ConnectionSecurity::StartTls => 143,
        };
        return format!("{server}:{port}");
    }
    server.to_owned()
}

/// The engine's transport enum for this account's connection security.
const fn engine_security(security: ConnectionSecurity) -> ImapSecurity {
    match security {
        ConnectionSecurity::ImplicitTls => ImapSecurity::ImplicitTls,
        ConnectionSecurity::StartTls => ImapSecurity::StartTls,
    }
}

/// The advertised mechanisms as one log-safe string. Protocol atoms, never a credential.
fn mechanisms(offer: &AuthOffer) -> String {
    if offer.mechanisms.is_empty() {
        "none".to_owned()
    } else {
        offer.mechanisms.join(" ")
    }
}

/// Reports what a submission server accepts, for the log only.
///
/// Called after an account is decided rather than as part of deciding, because IMAP is what
/// the account is built on and a submission server that disagrees is a provider
/// misconfiguration, not a different setup screen. Logging it at setup is what turns
/// "sending stopped working" months later into a line somebody can find.
pub async fn log_smtp_auth(smtp_host: &str, security: ConnectionSecurity, email: &str) {
    let Ok(tls) = account_tls() else { return };
    let host = host_of(smtp_host);
    let port = match security {
        ConnectionSecurity::ImplicitTls => 465,
        ConnectionSecurity::StartTls => 587,
    };
    let addr = if host_of(smtp_host) == smtp_host.trim() {
        format!("{host}:{port}")
    } else {
        smtp_host.trim().to_owned()
    };
    let ehlo = email
        .trim()
        .rsplit_once('@')
        .map_or("localhost", |(_, domain)| domain);
    match probe_smtp_auth(
        &addr,
        host,
        engine_security(security),
        ehlo,
        &tls.connector(),
    )
    .await
    {
        Ok(offer) => log::info!(
            "imap auth: submission at {host} offers [{}] (oauth: {}, password: {})",
            mechanisms(&offer),
            offer.oauth,
            offer.password,
        ),
        Err(err) => log::info!("imap auth: submission at {host} did not answer; {err}"),
    }
}

#[cfg(test)]
#[path = "imap_auth_tests.rs"]
mod tests;
