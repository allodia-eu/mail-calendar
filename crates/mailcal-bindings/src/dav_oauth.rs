//! Signing a mailbox's calendar and contacts in at their own server's provider, when the mailbox's
//! sign-in does not reach them (`docs/accounts.md` rule 18, `docs/mail-oauth.md` rule 11).
//!
//! The server is asked who signs it in, as a JMAP server is: an unauthenticated request answers
//! `401` and names its resource metadata (RFC 9728), which names the authorization server. That
//! issuer's tokens are sent to the server only where the profile allows them to go
//! (`draft-ietf-mailmaint-oauth-public` §3.6): its own list of protected resources, or its site.
//! A server naming an issuer anywhere else gets the password form, whatever it says, because a
//! token is the issuer's to aim and not the resource's to claim.
//!
//! The account the sign-in makes is the one [`add_linked_calendar_account`] makes with a
//! password, with the grant in place of the password; it is added, and has connected, before
//! the mailbox changes.
//!
//! [`add_linked_calendar_account`]: MailcalApp::add_linked_calendar_account

use std::sync::{Mutex, OnceLock};

use mailcal_account::{AccountConfig, Capability, OAuthGrant, Secret};
use mailcal_oauth::{DavUse, OAuthClient};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    AccountCapability, MailcalApp, MailcalError, account_registry::SignInConfig,
    account_sign_in::named_as,
};

/// The client name shown on the provider's consent screen.
const CLIENT_NAME: &str = "Allodia Mail & Calendar";

/// What [`MailcalApp::begin_separate_server_sign_in`] returns: the URL to open in the platform
/// auth session, and an opaque handle to pass back to the completion.
#[derive(uniffi::Record)]
pub struct SeparateServerSignInStart {
    /// The authorization URL to open in the platform auth session.
    pub authorization_url: String,
    /// An opaque handle for `complete_separate_server_sign_in`. **Transient; hold it in memory
    /// only.** It carries the PKCE verifier.
    pub pending: String,
}

/// The state carried across the browser hop, never persisted.
#[derive(Serialize, Deserialize)]
struct PendingDavLogin {
    account_id: String,
    calendar: bool,
    contacts: bool,
    login: String,
    client_id: String,
    client_secret: Option<String>,
    authorize_endpoint: String,
    token_endpoint: String,
    redirect_uri: String,
    scopes: Vec<String>,
    resource: Option<String>,
    /// The issuer the redirect's `iss` must name (RFC 9207), when the server sends one.
    issuer: Option<String>,
    authorization_server: String,
    protected_resources: Vec<String>,
    state: String,
    verifier: String,
}

impl PendingDavLogin {
    fn grant(&self, refresh_token: String) -> OAuthGrant {
        OAuthGrant {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone().map(Secret::new),
            refresh_token: Secret::new(refresh_token),
            authorize_endpoint: self.authorize_endpoint.clone(),
            token_endpoint: self.token_endpoint.clone(),
            redirect_uri: self.redirect_uri.clone(),
            scopes: self.scopes.clone(),
            resource: self.resource.clone(),
            issuer: self.issuer.clone(),
            authorization_server: Some(self.authorization_server.clone()),
            protected_resources: self.protected_resources.clone(),
        }
    }

    fn uses(&self) -> Vec<Capability> {
        [
            (self.calendar, Capability::Calendar),
            (self.contacts, Capability::Contacts),
        ]
        .into_iter()
        .filter_map(|(moved, capability)| moved.then_some(capability))
        .collect()
    }
}

/// The calendar account a sign-in would make for some uses of a mailbox, and where to ask.
struct Target {
    mailbox: AccountConfig,
    calendar: AccountConfig,
    uses: Vec<Capability>,
    /// The URL the server is asked who signs it in: the calendar's, else the address book's.
    url: String,
    /// The host of every server the grant must reach.
    hosts: Vec<String>,
    login: String,
}

impl Target {
    fn dav_uses(&self) -> Vec<DavUse> {
        self.uses
            .iter()
            .filter_map(|capability| match capability {
                Capability::Calendar => Some(DavUse::Calendars),
                Capability::Contacts => Some(DavUse::Contacts),
                _ => None,
            })
            .collect()
    }
}

/// The issuer that signs a separate server in, as its metadata and ours decide.
struct Issuer {
    metadata: mailcal_oauth::AuthServerMetadata,
    scopes: Vec<String>,
    resource: Option<String>,
}

#[uniffi::export]
impl MailcalApp {
    /// Whether the separate server holding `uses` of `account_id` can be signed in to through
    /// its own provider: it names an authorization server whose tokens may be sent to it, that
    /// offers its scopes and registers this app. Decides whether the form offers the sign-in
    /// beside its password field.
    ///
    /// **Blocking** (a few metadata requests); call it off the UI thread. Never an error: a
    /// server that cannot be asked offers the password, which is what works everywhere.
    pub fn separate_server_offers_sign_in(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
    ) -> bool {
        let Ok(target) = self.dav_target(&account_id, uses) else {
            return false;
        };
        self.runtime
            .block_on(|| async {
                let http = discovery_client()?;
                issuer_for(&http, &target).await
            })
            .is_ok()
    }

    /// Starts signing `uses` of `account_id` in at their server's own provider: discovers the
    /// issuer, registers this app with it, and returns the authorization URL.
    ///
    /// **Blocking**; call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] when the server offers no sign-in this app may use, and
    /// [`MailcalError::Config`] for an account that is not a standards mailbox or `uses` naming
    /// no server it has.
    pub fn begin_separate_server_sign_in(
        &self,
        account_id: String,
        uses: Vec<AccountCapability>,
        redirect_uri: String,
    ) -> Result<SeparateServerSignInStart, MailcalError> {
        let target = self.dav_target(&account_id, uses)?;
        let (issuer, client) = self.runtime.block_on(|| async {
            let http = discovery_client()?;
            let issuer = issuer_for(&http, &target).await?;
            let client = registered(&http, &issuer, &redirect_uri).await?;
            Ok::<_, MailcalError>((issuer, client))
        })?;
        let mut pending = PendingDavLogin {
            account_id,
            calendar: target.uses.contains(&Capability::Calendar),
            contacts: target.uses.contains(&Capability::Contacts),
            login: target.login,
            client_id: client.client_id,
            client_secret: client.client_secret,
            authorize_endpoint: issuer.metadata.authorization_endpoint.clone(),
            token_endpoint: issuer.metadata.token_endpoint.clone(),
            redirect_uri,
            scopes: issuer.scopes,
            resource: issuer.resource,
            issuer: issuer
                .metadata
                .issuer_parameter_supported
                .then(|| issuer.metadata.issuer.clone()),
            authorization_server: issuer.metadata.issuer,
            protected_resources: issuer.metadata.protected_resources,
            state: String::new(),
            verifier: String::new(),
        };
        let oauth = OAuthClient::new(pending.grant(String::new()).provider_config())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        let request = oauth.begin(Some(&pending.login));
        pending.state = request.state;
        pending.verifier = request.pkce.verifier().to_owned();
        log::info!("dav oauth: opening the authorization page; awaiting the redirect");
        Ok(SeparateServerSignInStart {
            authorization_url: request.authorization_url,
            pending: serde_json::to_string(&pending)
                .map_err(|err| MailcalError::Config(err.to_string()))?,
        })
    }

    /// Completes the sign-in [`begin_separate_server_sign_in`](Self::begin_separate_server_sign_in)
    /// started: exchanges the code, adds the calendar account with the grant, and moves the uses
    /// to it as [`add_linked_calendar_account`](Self::add_linked_calendar_account) does. Returns
    /// the new account's id.
    ///
    /// **Blocking**; call it off the UI thread.
    ///
    /// # Errors
    ///
    /// Returns [`MailcalError::Connect`] when the exchange fails or issues no refresh token,
    /// [`MailcalError::DifferentAccount`] when the server names another account, and the errors
    /// of the move. Both accounts are unchanged on every failure.
    pub fn complete_separate_server_sign_in(
        &self,
        account_id: String,
        pending: String,
        callback_url: String,
    ) -> Result<String, MailcalError> {
        let pending: PendingDavLogin =
            serde_json::from_str(&pending).map_err(|err| MailcalError::Config(err.to_string()))?;
        if pending.account_id != account_id {
            return Err(MailcalError::Config(
                "that sign-in was started for another account".to_owned(),
            ));
        }
        let oauth = OAuthClient::new(pending.grant(String::new()).provider_config())
            .map_err(|err| MailcalError::Config(err.to_string()))?;
        log::info!("dav oauth: redirect received; exchanging the code");
        let tokens = self
            .runtime
            .block_on(|| {
                oauth.complete(
                    &callback_url,
                    &pending.state,
                    &pending.verifier,
                    OffsetDateTime::now_utc(),
                )
            })
            .map_err(|err| {
                log::warn!("dav oauth: token exchange FAILED; {err}");
                crate::error::exchange_error(&err)
            })?;
        let refresh_token = tokens.refresh_token.ok_or_else(|| {
            log::warn!("dav oauth: the server issued no refresh token");
            MailcalError::Connect(
                "the server issued no refresh token (offline_access was requested)".to_owned(),
            )
        })?;
        named_as(tokens.username.as_deref(), &pending.login)?;
        log::info!(
            "dav oauth: sign-in complete; granted scope(s): [{}]",
            tokens.scope
        );
        let uses = pending
            .uses()
            .into_iter()
            .map(AccountCapability::from)
            .collect();
        let target = self.dav_target(&account_id, uses)?;
        let calendar = target
            .calendar
            .with_dav_grant(pending.grant(refresh_token.expose().to_owned()));
        if !calendar.uses_its_grant_does_not_reach().is_empty() {
            return Err(MailcalError::Config(
                "the sign-in does not reach the calendar or contacts it was for".to_owned(),
            ));
        }
        self.split_out(&account_id, calendar, target.mailbox, &target.uses)
    }
}

impl MailcalApp {
    /// The calendar account a sign-in would make for `uses` of the standards mailbox
    /// `account_id`.
    fn dav_target(
        &self,
        account_id: &str,
        uses: Vec<AccountCapability>,
    ) -> Result<Target, MailcalError> {
        let mailbox = match self.switchable(account_id)? {
            SignInConfig::Imap(config) if config.imap.is_some() => *config,
            _ => {
                return Err(MailcalError::Config(
                    "only a standards mailbox keeps its calendar on a server of its own".to_owned(),
                ));
            }
        };
        let uses: Vec<Capability> = uses.into_iter().map(Capability::from).collect();
        let calendar = mailbox
            .calendar_account_for(&uses)
            .ok_or_else(|| MailcalError::Config("nothing to sign in to".to_owned()))?;
        let urls: Vec<String> = [
            calendar
                .caldav
                .as_ref()
                .map(|caldav| caldav.base_url.as_str()),
            calendar
                .carddav
                .as_ref()
                .map(|carddav| carddav.base_url.as_str()),
        ]
        .into_iter()
        .flatten()
        .map(mailcal_account::normalize_caldav_base_url)
        .collect();
        let hosts = urls
            .iter()
            .filter_map(|url| url::Url::parse(url).ok()?.host_str().map(str::to_owned))
            .collect();
        let login = [
            calendar
                .caldav
                .as_ref()
                .map(|caldav| caldav.username.as_str()),
            calendar
                .carddav
                .as_ref()
                .map(|carddav| carddav.username.as_str()),
        ]
        .into_iter()
        .flatten()
        .next()
        .unwrap_or_default()
        .to_owned();
        let url = urls
            .into_iter()
            .next()
            .ok_or_else(|| MailcalError::Config("nothing to sign in to".to_owned()))?;
        Ok(Target {
            mailbox,
            calendar,
            uses,
            url,
            hosts,
            login,
        })
    }
}

fn discovery_client() -> Result<reqwest::Client, MailcalError> {
    mailcal_oauth::discovery_client_with(&mailcal_account::setup_trust_policy())
        .map_err(|err| MailcalError::Connect(err.to_string()))
}

/// The issuer `target`'s server names, when this app may use it for that server.
async fn issuer_for(http: &reqwest::Client, target: &Target) -> Result<Issuer, MailcalError> {
    // The URL stays out of the log: a DAV path routinely carries the person's address.
    log::info!("dav oauth: asking the server who signs it in (RFC 9728)");
    let protected = mailcal_oauth::discover_protected_resource(http, &target.url)
        .await
        .map_err(|err| unavailable(&format!("no resource metadata: {err}")))?;
    let metadata = mailcal_oauth::discover_auth_server(http, &protected.issuer)
        .await
        .map_err(|err| unavailable(&format!("the issuer's metadata: {err}")))?;
    log::info!(
        "dav oauth: issuer {}, register {}",
        metadata.issuer,
        metadata
            .registration_endpoint
            .as_deref()
            .unwrap_or("(none; sign-in cannot be offered)"),
    );
    let scopes = usable(&metadata, &target.hosts, &target.dav_uses())
        .map_err(|detail| unavailable(&detail))?;
    let resource = mailcal_oauth::resource_for(http, &metadata, &target.url)
        .await
        .ok()
        .flatten();
    Ok(Issuer {
        metadata,
        scopes,
        resource,
    })
}

/// The scopes to ask `metadata`'s issuer for, when its tokens may be sent to every one of
/// `hosts` (`draft-ietf-mailmaint-oauth-public` §3.6), it registers apps, and it offers every one
/// of `uses`; otherwise why not, for the log.
fn usable(
    metadata: &mailcal_oauth::AuthServerMetadata,
    hosts: &[String],
    uses: &[DavUse],
) -> Result<Vec<String>, String> {
    if let Some(host) = hosts.iter().find(|host| {
        !mailcal_oauth::token_may_reach(&metadata.issuer, &metadata.protected_resources, host)
    }) {
        return Err(format!("the issuer's tokens are not for {host}"));
    }
    if metadata.registration_endpoint.is_none() {
        return Err("the issuer registers no apps".to_owned());
    }
    let scopes = mailcal_oauth::select_dav_scopes(metadata, uses);
    if !mailcal_oauth::grants_dav_access(metadata, &scopes, uses) {
        return Err(format!(
            "no scope for what moves among [{}]",
            metadata.scopes_supported.join(", ")
        ));
    }
    Ok(scopes)
}

/// Registrations made this session, keyed by issuer and redirect, so a retried sign-in reuses
/// its client rather than leaving another registration behind at the provider.
static REGISTRATIONS: OnceLock<Mutex<Vec<(String, String, mailcal_oauth::RegisteredClient)>>> =
    OnceLock::new();

async fn registered(
    http: &reqwest::Client,
    issuer: &Issuer,
    redirect_uri: &str,
) -> Result<mailcal_oauth::RegisteredClient, MailcalError> {
    let cache = REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()));
    let key = (issuer.metadata.issuer.as_str(), redirect_uri);
    if let Some(client) = cache.lock().ok().and_then(|cache| {
        cache
            .iter()
            .find(|(cached, redirect, _)| (cached.as_str(), redirect.as_str()) == key)
            .map(|(_, _, client)| client.clone())
    }) {
        log::info!(
            "dav oauth: reusing client id {} registered earlier",
            client.client_id
        );
        return Ok(client);
    }
    log::info!("dav oauth: registering this install as a client (RFC 7591)");
    let client = mailcal_oauth::register_client(
        http,
        &issuer.metadata,
        CLIENT_NAME,
        redirect_uri,
        &issuer.scopes,
    )
    .await
    .map_err(|err| unavailable(&format!("registration was refused: {err}")))?;
    log::info!("dav oauth: registered client id {}", client.client_id);
    if let Ok(mut cache) = cache.lock() {
        cache.push((key.0.to_owned(), redirect_uri.to_owned(), client.clone()));
    }
    Ok(client)
}

/// The one answer a person gets for any of these: the server's sign-in is not available here,
/// and the password works. Which step gave up stays in the log.
fn unavailable(detail: &str) -> MailcalError {
    log::info!("dav oauth: sign-in unavailable; {detail}");
    MailcalError::Connect(format!("this server does not offer sign-in: {detail}"))
}

#[cfg(test)]
#[path = "dav_oauth_tests.rs"]
mod tests;
