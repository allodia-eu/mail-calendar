//! "Sign in with your provider" for an **IMAP/SMTP** account, over the FFI.
//!
//! The shape mirrors [`crate::jmap_oauth`], because it is the same flow against the same
//! kind of server: one we have never met, discovered from the standards rather than
//! integrated by hand. The browser half belongs to the host, since capturing a
//! custom-scheme redirect is platform-specific.
//!
//! One step differs, and it is the reason this module exists rather than a shared one. A
//! JMAP server *is* an HTTP resource, so an unauthenticated request to it answers `401`
//! and names its authorization server (RFC 9728). An IMAP server has no such surface: the
//! issuer has to come from the provider's own autoconfig or from a well-known probe of the
//! domains the account involves, and whether OAuth is on offer at all comes from the mail
//! server's own pre-authentication capability line. That decision is
//! [`MailcalApp::imap_auth_options`]; everything after it is the flow JMAP already runs.
//!
//! # What the host does with the result
//!
//! [`MailcalApp::complete_imap_login`] returns the same account-config TOML the password
//! form produces, so a client adds and stores it through the path it already has. A
//! **rotated** refresh token is re-persisted through the host's
//! [`AccountCredentialStore`](crate::AccountCredentialStore), as for every other OAuth
//! account; without one, an account whose server rotates would die at the next launch.

use mailcal_account::{
    AccountSetup, ConnectionSecurity as AccountSecurity, ImapAuth, ImapAuthQuery, OAuthGrant,
    Secret, SetupCredential,
};
use mailcal_oauth::OAuthClient;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{ConnectionSecurity, MailcalApp, MailcalError};

mod discovery;

use discovery::discover_registration;

/// The client name shown on the provider's consent screen. The product name, per the brand
/// rule: this is user-facing copy on someone else's page.
const CLIENT_NAME: &str = "Allodia Mail & Calendar";

/// What setup should ask this account for, once the server has been asked.
///
/// Deliberately not a single "supports OAuth" flag. The three cases need three different
/// screens, and the middle one is the whole reason: a provider that offers OAuth only to
/// applications it registered in advance is not the same as one that offers none, and
/// showing the same "type your password" form for both leaves the user with no idea why the
/// sign-in button their friend has is missing.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ImapAuthOffer {
    /// Offer "sign in with your provider" as the primary action, with the password field
    /// behind a secondary control when `password_also_works`.
    SignIn {
        /// The authorization server the sign-in will use. Shown so a cautious user can see
        /// where they are about to be sent, and carried in the diagnostic log.
        issuer: String,
        /// The provider's name for the button, when this build holds a registration naming
        /// one. `None` for a server discovered from the standards, where the button names no
        /// provider because nothing told us one.
        provider_label: Option<String>,
        /// Whether a password is still accepted, so the client can offer "use a password
        /// instead". `false` where that link would dead-end.
        password_also_works: bool,
    },
    /// The server takes an OAuth token, but only from an application registered with it in
    /// advance, and this build carries no registration for it. The client says so and offers
    /// the password route.
    RegistrationNeeded {
        /// Whether a password still works. Almost always true.
        password_also_works: bool,
    },
    /// No OAuth on offer, or the server did not answer: the password form, unchanged.
    Password,
}

impl From<ImapAuth> for ImapAuthOffer {
    fn from(auth: ImapAuth) -> Self {
        match auth {
            ImapAuth::SignIn {
                issuer,
                provider_label,
                password_also_works,
            } => Self::SignIn {
                issuer,
                provider_label,
                password_also_works,
            },
            ImapAuth::RegistrationNeeded {
                password_also_works,
            } => Self::RegistrationNeeded {
                password_also_works,
            },
            ImapAuth::Password => Self::Password,
        }
    }
}

/// The account a sign-in is being started for: the same fields the password form collects,
/// minus the secret it will not be asked for.
#[derive(uniffi::Record, Debug, Clone)]
pub struct ImapLoginRequest {
    /// The account's email address.
    pub email: String,
    /// The IMAP server (host, or `host:port`).
    pub imap_host: String,
    /// The SMTP server, when one was found; mail-send stays unconfigured otherwise.
    pub smtp_host: Option<String>,
    /// A CalDAV endpoint to attach, when the user accepted the discovered calendar.
    pub caldav_base_url: Option<String>,
    /// How the IMAP connection is secured. `None` means implicit TLS.
    #[uniffi(default = None)]
    pub imap_security: Option<ConnectionSecurity>,
    /// How the SMTP connection is secured. `None` means implicit TLS.
    #[uniffi(default = None)]
    pub smtp_security: Option<ConnectionSecurity>,
    /// The issuer the provider's own autoconfig named, passed straight back from
    /// [`SetupRecommendation::Imap`](crate::SetupRecommendation).
    #[uniffi(default = None)]
    pub oauth_issuer: Option<String>,
}

/// What [`MailcalApp::begin_imap_login`] returns: the URL to open in the platform auth
/// session, and an opaque handle to pass back to `complete_imap_login`.
#[derive(uniffi::Record)]
pub struct ImapLoginStart {
    /// The authorization URL to open in the platform auth session.
    pub authorization_url: String,
    /// An opaque handle (the account's servers, the discovered endpoints, the registered
    /// client id and the PKCE verifier). **Transient; hold it in memory only.**
    pub pending: String,
}

/// The transient state carried between begin and complete. Round-tripped through the host as
/// the opaque `pending` handle, and never persisted; it holds the PKCE verifier.
#[derive(Serialize, Deserialize)]
struct PendingImapLogin {
    email: String,
    imap_host: String,
    smtp_host: Option<String>,
    caldav_base_url: Option<String>,
    imap_starttls: bool,
    smtp_starttls: bool,
    client_id: String,
    client_secret: Option<String>,
    authorize_endpoint: String,
    token_endpoint: String,
    redirect_uri: String,
    scopes: Vec<String>,
    /// The issuer the redirect's `iss` must name (RFC 9207), when the server advertised that
    /// it sends one. Carried across the hop because the check happens on the way back.
    issuer: Option<String>,
    state: String,
    verifier: String,
}

impl PendingImapLogin {
    /// The grant this pending login becomes, given the refresh token it was exchanged for.
    ///
    /// Kept beside the struct so the fields carried across the browser hop and the fields
    /// persisted afterwards cannot drift: a `redirect_uri` that stops being replayed, or a
    /// dropped `issuer`, both fail later and somewhere else.
    fn into_grant(self, refresh_token: String) -> (AccountSetup, OAuthGrant) {
        let grant = OAuthGrant {
            client_id: self.client_id,
            client_secret: self.client_secret.map(Secret::new),
            refresh_token: Secret::new(refresh_token),
            authorize_endpoint: self.authorize_endpoint,
            token_endpoint: self.token_endpoint,
            redirect_uri: self.redirect_uri,
            scopes: self.scopes,
            // An IMAP endpoint is not an HTTPS URL and the profile defines no URI form for
            // one, so there is no RFC 8707 target to name. A server that scopes tokens by
            // resource applies its default; inventing an `imap://…` would risk
            // `invalid_target` on every refresh.
            resource: None,
            issuer: self.issuer,
        };
        let setup = AccountSetup {
            imap_host: self.imap_host,
            username: self.email,
            credential: SetupCredential::Grant(Box::new(grant.clone())),
            smtp_host: self.smtp_host,
            caldav_base_url: self.caldav_base_url,
            imap_security: security(self.imap_starttls),
            smtp_security: security(self.smtp_starttls),
        };
        (setup, grant)
    }
}

/// The account-layer security for a carried STARTTLS flag.
const fn security(starttls: bool) -> AccountSecurity {
    if starttls {
        AccountSecurity::StartTls
    } else {
        AccountSecurity::ImplicitTls
    }
}

/// Whether a client-supplied security is STARTTLS. `None` is implicit TLS, the default the
/// manual form uses.
fn is_starttls(security: Option<ConnectionSecurity>) -> bool {
    matches!(security, Some(ConnectionSecurity::StartTls))
}

/// The account-layer security for a client-supplied one.
fn account_security(security: Option<ConnectionSecurity>) -> AccountSecurity {
    security.map(Into::into).unwrap_or_default()
}

#[uniffi::export]
impl MailcalApp {
    /// What setup should ask this account for: sign in with the provider, explain that the
    /// provider admits only pre-registered applications, or ask for a password.
    ///
    /// Asks the **mail server** first, before any OAuth discovery: a domain can publish an
    /// authorization server for its web sessions and take only a password on IMAP, and a
    /// sign-in offered on that evidence mints a token the mailbox refuses.
    ///
    /// **Blocking** (one TLS connection to the mail server, then up to a few short HTTPS
    /// requests); call it off the main thread, exactly like `detect_account_settings`. Never
    /// throws: every unanswered question resolves to [`ImapAuthOffer::Password`], which works
    /// everywhere.
    pub fn imap_auth_options(&self, request: ImapLoginRequest) -> ImapAuthOffer {
        let query = ImapAuthQuery {
            imap_host: request.imap_host,
            imap_security: account_security(request.imap_security),
            email: request.email,
            autoconfig_issuer: request.oauth_issuer,
        };
        self.runtime
            .block_on(mailcal_account::decide_imap_auth(&query))
            .into()
    }

    /// Starts an IMAP OAuth sign-in: finds the authorization server, registers this install
    /// as a client where the server offers open registration, and builds the PKCE
    /// authorization URL to open.
    ///
    /// `redirect_uri` is the platform's registered custom scheme, whose redirect the host's
    /// auth session captures.
    ///
    /// **Blocking** (several network round trips); call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] when no usable authorization server is found. That
    /// is an expected outcome rather than a defect, and the same one
    /// [`imap_auth_options`](Self::imap_auth_options) reports as
    /// [`ImapAuthOffer::RegistrationNeeded`]: the caller falls back to the password field.
    pub fn begin_imap_login(
        &self,
        request: ImapLoginRequest,
        redirect_uri: String,
    ) -> Result<ImapLoginStart, MailcalError> {
        let query = ImapAuthQuery {
            imap_host: request.imap_host.clone(),
            imap_security: account_security(request.imap_security),
            email: request.email.clone(),
            autoconfig_issuer: request.oauth_issuer.clone(),
        };
        log::info!("imap oauth: sign-in requested for the server the account dials");
        let registration = self
            .runtime
            .block_on(discover_registration(&query, &redirect_uri))?;

        let pending = PendingImapLogin {
            email: request.email,
            imap_host: request.imap_host,
            smtp_host: request.smtp_host,
            caldav_base_url: request.caldav_base_url,
            imap_starttls: is_starttls(request.imap_security),
            smtp_starttls: is_starttls(request.smtp_security),
            client_id: registration.client_id,
            client_secret: registration.client_secret,
            authorize_endpoint: registration.authorize_endpoint,
            token_endpoint: registration.token_endpoint,
            redirect_uri,
            scopes: registration.scopes,
            issuer: registration.expected_issuer,
            state: String::new(),
            verifier: String::new(),
        };
        start_login(pending)
    }

    /// Completes an IMAP OAuth sign-in: validates the redirect, exchanges the code for
    /// tokens, and returns the account-config TOML.
    ///
    /// Byte-for-byte the shape the password form produces, plus an `[imap.oauth]` section and
    /// no stored secret, so the host adds it with the code path it already has.
    ///
    /// **Blocking**; call it off the main thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Config`] if `pending` is malformed or the config will not
    /// serialize, or [`MailcalError::Connect`] if the user cancelled, the exchange failed, or
    /// the server issued no refresh token (without one the account would break within the
    /// hour).
    pub fn complete_imap_login(
        &self,
        pending: String,
        callback_url: String,
    ) -> Result<String, MailcalError> {
        let pending: PendingImapLogin =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        let grant_shell = grant_shell(&pending);
        let oauth = OAuthClient::new(grant_shell.provider_config())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!(
            "imap oauth: redirect received; exchanging the code at {}",
            pending.token_endpoint,
        );
        let tokens = self
            .runtime
            .block_on(oauth.complete(
                &callback_url,
                &pending.state,
                &pending.verifier,
                OffsetDateTime::now_utc(),
            ))
            .map_err(|err| {
                // The server's own machine-readable reason, which is the single most useful
                // line in the whole flow for support: `invalid_grant` means the code was
                // stale or replayed, and a callback rejection names which check failed.
                log::warn!("imap oauth: token exchange FAILED; {err}");
                MailcalError::Connect(err.to_string())
            })?;
        // `offline_access` is always requested, so a refresh token is mandatory: without one
        // the account stops syncing about an hour after setup, which is far worse to diagnose
        // than failing here.
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            log::warn!(
                "imap oauth: exchange succeeded but the server issued NO refresh token; granted scope(s): [{}]",
                tokens.scope,
            );
            MailcalError::Connect(
                "the server issued no refresh token (offline_access was requested)".to_owned(),
            )
        })?;
        log::info!(
            "imap oauth: sign-in complete; access token valid until {}, granted scope(s): [{}]",
            tokens.expires_at,
            tokens.scope,
        );
        let (setup, _grant) = pending.into_grant(refresh_token.expose().to_owned());
        mailcal_account::build_config_toml(&setup)
            .map_err(|err| MailcalError::Config(err.to_string()))
    }
}

/// The grant a pending login describes, before it has a refresh token: used to build the
/// OAuth client for the exchange, so the endpoints, redirect and issuer come from one place
/// rather than being restated at the call site.
fn grant_shell(pending: &PendingImapLogin) -> OAuthGrant {
    OAuthGrant {
        client_id: pending.client_id.clone(),
        client_secret: pending.client_secret.clone().map(Secret::new),
        refresh_token: Secret::new(String::new()),
        authorize_endpoint: pending.authorize_endpoint.clone(),
        token_endpoint: pending.token_endpoint.clone(),
        redirect_uri: pending.redirect_uri.clone(),
        scopes: pending.scopes.clone(),
        resource: None,
        issuer: pending.issuer.clone(),
    }
}

/// Builds the PKCE authorization request for a pending login and packs everything the
/// completion needs into the opaque handle.
fn start_login(mut pending: PendingImapLogin) -> Result<ImapLoginStart, MailcalError> {
    let oauth = OAuthClient::new(grant_shell(&pending).provider_config())
        .map_err(|err| MailcalError::Config(err.to_string()))?;
    // The address is known, so target it rather than making the user pick an account.
    let request = oauth.begin(Some(&pending.email));
    pending.state = request.state;
    pending.verifier = request.pkce.verifier().to_owned();
    log::info!("imap oauth: opening the authorization page; awaiting the redirect");
    Ok(ImapLoginStart {
        authorization_url: request.authorization_url,
        pending: serde_json::to_string(&pending)
            .map_err(|err| MailcalError::Config(err.to_string()))?,
    })
}

#[cfg(test)]
#[path = "imap_oauth_tests.rs"]
mod tests;
