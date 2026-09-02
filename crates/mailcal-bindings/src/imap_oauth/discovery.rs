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
    redirect_uri: &str,
) -> Result<Registration, MailcalError> {
    if let Some(registration) = static_registration(query) {
        return Ok(registration);
    }
    let http =
        mailcal_oauth::discovery_client().map_err(|err| MailcalError::Connect(err.to_string()))?;
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

    let scopes = mailcal_oauth::select_scopes(&metadata);
    log::info!(
        "imap oauth: server offers {} scope(s); requesting {}: [{}]",
        metadata.scopes_supported.len(),
        scopes.len(),
        scopes.join(", "),
    );
    if !mailcal_oauth::grants_mail_access(&metadata, &scopes) {
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
    Ok(Registration {
        client_id: client.client_id,
        client_secret: client.client_secret,
        authorize_endpoint: metadata.authorization_endpoint,
        token_endpoint: metadata.token_endpoint,
        scopes,
        expected_issuer: metadata
            .issuer_parameter_supported
            .then_some(metadata.issuer),
    })
}

/// The registration this **build** carries for the account's provider, if any.
///
/// Nothing is discovered and nothing is registered on this path: the provider issued the
/// client id in advance, which is precisely why such providers need an entry at all.
fn static_registration(query: &ImapAuthQuery) -> Option<Registration> {
    let host =
        query
            .imap_host
            .trim()
            .rsplit_once(':')
            .map_or(query.imap_host.trim(), |(host, port)| {
                if port.bytes().all(|byte| byte.is_ascii_digit()) {
                    host
                } else {
                    query.imap_host.trim()
                }
            });
    let provider = mailcal_oauth::provider_for_host(&mailcal_oauth::static_mail_providers(), host)?;
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
    })
}

/// The one message a user ever sees for any of these: this provider's sign-in is not
/// available here. Which step gave up stays in the diagnostic log, where it is useful, rather
/// than on a setup screen, where it is not.
fn unavailable(detail: &str) -> MailcalError {
    log::info!("imap oauth: sign-in unavailable; {detail}");
    MailcalError::Connect(format!("this server does not offer sign-in: {detail}"))
}
