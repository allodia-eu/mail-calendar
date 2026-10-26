//! Finding the authorization server an IMAP account signs in at, and getting a client id for
//! it. Split from the FFI surface in the module root so both stay under the size cap.
//!
//! Two ways to arrive at a client id, and the order matters. A registration this **build**
//! carries is used as-is: the provider issued it in advance, there is nothing to register,
//! and asking would fail. Otherwise the server's own metadata is read and this install
//! registers itself (RFC 8414 → RFC 7591), which is what a server publishing
//! `registration_endpoint` is inviting.
//!
//! Everything here logs. That is the point: this runs against servers nobody on the team has
//! seen, so a support request has to be answerable from the user's attached log alone. All of
//! it is server configuration (URLs, scope names, the issued client id), never the user's
//! address, secret, or token ([`docs/logging.md`](../../../docs/logging.md)).

use std::sync::{Mutex, OnceLock};

use mailcal_account::ImapAuthQuery;

use super::CLIENT_NAME;
use crate::MailcalError;

/// Everything an authorization request needs, however the client id was arrived at.
pub(super) struct Registration {
    pub(super) client_id: String,
    pub(super) client_secret: Option<String>,
    pub(super) authorize_endpoint: String,
    pub(super) token_endpoint: String,
    pub(super) scopes: Vec<String>,
    /// The issuer the redirect's `iss` must name (RFC 9207), when the server advertised that
    /// it sends one. `None` for a static registration, whose provider we have not read
    /// metadata from, and for a server that does not advertise the parameter.
    pub(super) expected_issuer: Option<String>,
    /// The issuer identifier, kept with the grant so a connect can tell which hosts its tokens
    /// may reach (`draft-ietf-mailmaint-oauth-public` §3.6).
    pub(super) authorization_server: String,
    /// The issuer's `protected_resources`, kept for the same reason. Empty when it lists none.
    pub(super) protected_resources: Vec<String>,
    /// The endpoints to name in the authorization request (§3.4), each one vouched for.
    pub(super) resources: Vec<String>,
}

/// What the account will present its token to, for [`discover_registration`] to decide which of
/// them the authorization request names.
pub(super) struct Endpoints<'a> {
    pub(super) smtp_host: Option<&'a str>,
    pub(super) caldav: Option<&'a str>,
    pub(super) carddav: Option<&'a str>,
}

/// Registrations already made this session, keyed by `(issuer, redirect_uri)`.
///
/// Dynamic registration mints a **new** client id every time it is called, so without this
/// every cancelled or failed attempt would leave another orphaned registration on the user's
/// account: three taps of a flaky sign-in, three clients. Reusing the one already held is
/// also simply correct: same software, same install, same server.
///
/// Session-scoped on purpose. Once a sign-in *completes* the client id is persisted with the
/// account and no further registration happens for it, so the only case left is repeated
/// attempts before that, which is exactly the case that leaks.
static REGISTRATIONS: OnceLock<Mutex<Vec<(String, String, mailcal_oauth::RegisteredClient)>>> =
    OnceLock::new();

/// The client already registered for `issuer` + `redirect_uri` this session, if any.
fn cached(issuer: &str, redirect_uri: &str) -> Option<mailcal_oauth::RegisteredClient> {
    let cache = REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()));
    let cache = cache.lock().ok()?;
    cache
        .iter()
        .find(|(cached_issuer, cached_redirect, _)| {
            cached_issuer == issuer && cached_redirect == redirect_uri
        })
        .map(|(_, _, client)| client.clone())
}

/// Remembers `client` for `issuer` + `redirect_uri` so a retry reuses it.
fn remember(issuer: &str, redirect_uri: &str, client: &mailcal_oauth::RegisteredClient) {
    let cache = REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.push((issuer.to_owned(), redirect_uri.to_owned(), client.clone()));
    }
}

/// Resolves a usable client for the account described by `query`.
///
/// # Errors
///
/// Returns [`MailcalError::Connect`] when no authorization server can be found, when it
/// offers no registration, or when the registration is refused. All of them mean the same
/// thing to a caller: this provider's sign-in is not available to us, use the password field.
pub(super) async fn discover_registration(
    query: &ImapAuthQuery,
    endpoints: &Endpoints<'_>,
    redirect_uri: &str,
) -> Result<Registration, MailcalError> {
    if let Some(registration) = static_registration(query) {
        return Ok(registration);
    }
    let http = mailcal_oauth::discovery_client_with(&mailcal_account::setup_trust_policy())
        .map_err(|err| MailcalError::Connect(err.to_string()))?;
    // The same metadata the setup screen's decision was made from, not a second reading of
    // it: two fetches could disagree, and the one the user was shown is the one to honour.
    let metadata = mailcal_account::imap_issuer(query)
        .await
        .ok_or_else(|| unavailable("this server names no authorization server we can use"))?;
    log::info!(
        "imap oauth: authorize {}, token {}, register {}",
        metadata.authorization_endpoint,
        metadata.token_endpoint,
        metadata
            .registration_endpoint
            .as_deref()
            .unwrap_or("(none; sign-in cannot be offered)"),
    );

    let scopes = mailcal_oauth::select_scopes(&metadata, mailcal_oauth::MailProtocol::Imap);
    log::info!(
        "imap oauth: server offers {} scope(s); requesting {}: [{}]",
        metadata.scopes_supported.len(),
        scopes.len(),
        scopes.join(", "),
    );
    if !mailcal_oauth::grants_mail_access(&metadata, &scopes, mailcal_oauth::MailProtocol::Imap) {
        log::warn!(
            "imap oauth: gave up: no mail scope among [{}]; the password field still works",
            metadata.scopes_supported.join(", "),
        );
        return Err(unavailable("its sign-in does not offer mail access"));
    }

    let client = if let Some(client) = cached(&metadata.issuer, redirect_uri) {
        log::info!(
            "imap oauth: reusing client id {} registered earlier this session",
            client.client_id,
        );
        client
    } else {
        log::info!("imap oauth: registering this install as a client (RFC 7591)");
        let client =
            mailcal_oauth::register_client(&http, &metadata, CLIENT_NAME, redirect_uri, &scopes)
                .await
                .map_err(|err| unavailable(&format!("registration was refused: {err}")))?;
        log::info!(
            "imap oauth: registered client id {} (server issued a secret: {})",
            client.client_id,
            client.client_secret.is_some(),
        );
        remember(&metadata.issuer, redirect_uri, &client);
        client
    };
    let resources = resources_to_name(&http, &metadata, query, endpoints, &scopes).await;
    Ok(Registration {
        client_id: client.client_id,
        client_secret: client.client_secret,
        authorize_endpoint: metadata.authorization_endpoint,
        token_endpoint: metadata.token_endpoint,
        scopes,
        expected_issuer: metadata
            .issuer_parameter_supported
            .then(|| metadata.issuer.clone()),
        authorization_server: metadata.issuer,
        protected_resources: metadata.protected_resources,
        resources,
    })
}

/// The registration this **build** carries for the account's provider, if any.
///
/// Nothing is discovered and nothing is registered on this path: the provider issued the
/// client id in advance, which is precisely why such providers need an entry at all.
fn static_registration(query: &ImapAuthQuery) -> Option<Registration> {
    let provider = mailcal_oauth::provider_for_host(
        &mailcal_oauth::static_mail_providers(),
        host_only(&query.imap_host),
    )?;
    log::info!(
        "imap oauth: using this build's registration for {} at {}",
        provider.label,
        provider.issuer,
    );
    Some(Registration {
        client_id: provider.client_id,
        client_secret: provider.client_secret,
        authorize_endpoint: provider.authorize_endpoint.to_owned(),
        token_endpoint: provider.token_endpoint.to_owned(),
        scopes: provider.scopes.iter().map(|s| (*s).to_owned()).collect(),
        // No metadata was read, so nothing said whether this server sends `iss`. Asserting an
        // issuer we have not been told about would reject every callback from a server that
        // sends none.
        expected_issuer: None,
        authorization_server: provider.issuer.to_owned(),
        protected_resources: Vec::new(),
        resources: Vec::new(),
    })
}

/// The endpoints the authorization request names (`draft-ietf-mailmaint-oauth-public` §3.4):
/// each only where something from the issuer's side vouches for it, because a server that
/// supports resource indicators refuses an unknown one and the whole sign-in with it.
///
/// IMAP and SMTP are named only when the issuer lists them: the profile's bare host name is
/// refused by servers that otherwise implement it (Stalwart answers it `401`, Fastmail
/// `invalid_target`). A DAV endpoint is named when the grant could reach it at all (its scope
/// requested, its host one the issuer's tokens are for) and its own RFC 9728 metadata names
/// this issuer and covers it, or the issuer's list does.
async fn resources_to_name(
    http: &reqwest::Client,
    metadata: &mailcal_oauth::AuthServerMetadata,
    query: &ImapAuthQuery,
    endpoints: &Endpoints<'_>,
    scopes: &[String],
) -> Vec<String> {
    // §3.4: "If two endpoints result in the same value, the value MUST only be included once."
    let mut named: Vec<String> = Vec::new();
    let mut name = |resource: String| {
        if !named.contains(&resource) {
            named.push(resource);
        }
    };
    for host in
        std::iter::once(host_only(&query.imap_host)).chain(endpoints.smtp_host.map(host_only))
    {
        if let Some(entry) = mailcal_oauth::covering_entry(&metadata.protected_resources, host) {
            name(entry);
        }
    }
    let dav = [
        (
            endpoints.caldav,
            mailcal_oauth::DavUse::Calendars,
            "calendar",
        ),
        (
            endpoints.carddav,
            mailcal_oauth::DavUse::Contacts,
            "contacts",
        ),
    ];
    // The address book usually shares the calendar's URL, so its metadata is asked for once.
    let mut asked: Vec<(String, Option<String>)> = Vec::new();
    // The URLs stay out of the log: a DAV path routinely carries the person's address.
    for (url, dav_use, label) in dav {
        let Some(url) = url.map(mailcal_account::normalize_caldav_base_url) else {
            continue;
        };
        let reachable = mailcal_oauth::requests_dav_use(scopes, dav_use)
            && url::Url::parse(&url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .is_some_and(|host| {
                    mailcal_oauth::token_may_reach(
                        &metadata.issuer,
                        &metadata.protected_resources,
                        &host,
                    )
                });
        if !reachable {
            log::info!("imap oauth: the {label} server is not one this sign-in reaches; not named");
            continue;
        }
        let resource = if let Some((_, resource)) = asked.iter().find(|(asked, _)| *asked == url) {
            resource.clone()
        } else {
            let resource = match mailcal_oauth::resource_for(http, metadata, &url).await {
                Ok(resource) => resource,
                Err(_) => {
                    log::info!(
                        "imap oauth: the {label} server published no usable resource metadata"
                    );
                    None
                }
            };
            asked.push((url, resource.clone()));
            resource
        };
        match resource {
            Some(resource) => name(resource),
            None => log::info!("imap oauth: nothing vouches for the {label} server; not named"),
        }
    }
    log::info!("imap oauth: naming {} resource(s)", named.len());
    named
}

/// The endpoints signing an account in again names: [`resources_to_name`] over the metadata of
/// `issuer`, read again. None when there is no issuer to read or it cannot be read, which is
/// what a refresh names.
pub(super) async fn resources_again(
    issuer: Option<&str>,
    query: &ImapAuthQuery,
    endpoints: &Endpoints<'_>,
    scopes: &[String],
) -> Vec<String> {
    let Some(issuer) = issuer else {
        return Vec::new();
    };
    let Ok(http) = mailcal_oauth::discovery_client_with(&mailcal_account::setup_trust_policy())
    else {
        return Vec::new();
    };
    match mailcal_oauth::discover_auth_server(&http, issuer).await {
        Ok(metadata) => resources_to_name(&http, &metadata, query, endpoints, scopes).await,
        Err(err) => {
            log::info!("imap oauth: the issuer's metadata was not read again ({err}); naming none");
            Vec::new()
        }
    }
}

/// The host of a `host` or `host:port` server field.
pub(crate) fn host_only(server: &str) -> &str {
    let server = server.trim();
    match server.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => server,
    }
}

/// The one message a user ever sees for any of these: this provider's sign-in is not
/// available here. Which step gave up stays in the diagnostic log, where it is useful, rather
/// than on a setup screen, where it is not.
fn unavailable(detail: &str) -> MailcalError {
    log::info!("imap oauth: sign-in unavailable; {detail}");
    MailcalError::Connect(format!("this server does not offer sign-in: {detail}"))
}
