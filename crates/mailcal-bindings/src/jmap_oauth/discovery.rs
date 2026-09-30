//! The RFC 9728 → 8414 → 7591 chain behind [`MailcalApp::begin_jmap_login`], split from the FFI
//! surface in the module root so both stay under the 500-line cap.
//!
//! Everything here logs. That is the point: this runs against servers nobody on the team has
//! seen, so a support request has to be answerable from the user's attached log alone; how far
//! did it get, and what did the server say. All of it is server configuration (URLs, scope
//! names, the issued client id); never the user's address, secret, or token
//! ([`docs/logging.md`](../../../docs/logging.md)).

use std::sync::{Mutex, OnceLock};

use super::{CLIENT_NAME, SESSION_PATH};
use crate::MailcalError;

/// Registrations already made this session, keyed by `(issuer, redirect_uri)`.
///
/// Dynamic registration mints a **new** client id every time it is called, so without this every
/// cancelled or failed attempt would leave another orphaned client registration behind on the
/// user's account; three taps of a flaky sign-in, three registrations. Reusing the one we
/// already hold is also simply correct: same software, same install, same server.
///
/// Session-scoped on purpose. Once a sign-in *completes*, the client id is persisted with the
/// account and no further registration ever happens for it (`OAuthGrant::client_id`), so the only
/// case left to cover is repeated attempts before that, which is exactly the case that leaked.
/// Making it survive a relaunch would need a new host storage port for a credential that belongs
/// to no account yet, that is deliberately not built here.
static REGISTRATIONS: OnceLock<Mutex<Vec<(String, String, mailcal_oauth::RegisteredClient)>>> =
    OnceLock::new();

/// The client already registered for `issuer` + `redirect_uri` this session, if any.
fn cached_registration(
    issuer: &str,
    redirect_uri: &str,
) -> Option<mailcal_oauth::RegisteredClient> {
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
fn cache_registration(issuer: &str, redirect_uri: &str, client: &mailcal_oauth::RegisteredClient) {
    let cache = REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut cache) = cache.lock() {
        cache.push((issuer.to_owned(), redirect_uri.to_owned(), client.clone()));
    }
}

/// Everything the discovery chain produced, ready to build an authorization request from.
pub(super) struct Discovered {
    pub(super) metadata: mailcal_oauth::AuthServerMetadata,
    pub(super) scopes: Vec<String>,
    pub(super) client: mailcal_oauth::RegisteredClient,
    /// The RFC 8707 resource indicator the server published, if any.
    pub(super) resource: Option<String>,
}

/// Runs the RFC 9728 → 8414 → 7591 chain for the JMAP server at `base_url`.
///
/// Every step logs what it is about to do **and what it found**, because this runs against
/// servers we have never seen and a support request has to be diagnosable from the log alone;
/// "how far did it get, and what did the server say"; without anyone reproducing it. Everything
/// logged is server configuration (URLs, scope names, the issued client id); never the user's
/// address, secret, or token (`docs/logging.md`).
pub(super) async fn discover_and_register(
    base_url: &str,
    redirect_uri: &str,
) -> Result<Discovered, MailcalError> {
    let http =
        mailcal_oauth::discovery_client().map_err(|err| MailcalError::Connect(err.to_string()))?;
    let session_url = format!("{}{SESSION_PATH}", base_url.trim_end_matches('/'));

    log::info!("jmap oauth: step 1/4; probing {session_url} for resource metadata (RFC 9728)");
    let protected = mailcal_oauth::discover_protected_resource(&http, &session_url)
        .await
        .map_err(discovery_error)?;
    log::info!(
        "jmap oauth: step 1/4 ok, issuer {}, resource indicator {}",
        protected.issuer,
        protected.resource.as_deref().unwrap_or("(none published)"),
    );

    log::info!(
        "jmap oauth: step 2/4; reading authorization-server metadata for {} (RFC 8414)",
        protected.issuer,
    );
    let metadata = mailcal_oauth::discover_auth_server(&http, &protected.issuer)
        .await
        .map_err(discovery_error)?;
    log::info!(
        "jmap oauth: step 2/4 ok; authorize {}, token {}, register {}",
        metadata.authorization_endpoint,
        metadata.token_endpoint,
        metadata
            .registration_endpoint
            .as_deref()
            .unwrap_or("(none; sign-in cannot be offered)"),
    );

    let scopes = mailcal_oauth::select_scopes(&metadata);
    log::info!(
        "jmap oauth: step 3/4; server offers {} scope(s); requesting {}: [{}]",
        metadata.scopes_supported.len(),
        scopes.len(),
        scopes.join(", "),
    );
    if !mailcal_oauth::grants_mail_access(&metadata, &scopes) {
        log::warn!(
            "jmap oauth: step 3/4 gave up: no mail scope among [{}]; the manual secret still works",
            metadata.scopes_supported.join(", "),
        );
        return Err(MailcalError::Connect(
            "this server's sign-in does not offer mail access".to_owned(),
        ));
    }

    let client = if let Some(client) = cached_registration(&protected.issuer, redirect_uri) {
        log::info!(
            "jmap oauth: step 4/4; reusing client id {} registered earlier this session",
            client.client_id,
        );
        client
    } else {
        log::info!("jmap oauth: step 4/4; registering this install as a client (RFC 7591)");
        let client =
            mailcal_oauth::register_client(&http, &metadata, CLIENT_NAME, redirect_uri, &scopes)
                .await
                .map_err(discovery_error)?;
        log::info!(
            "jmap oauth: step 4/4 ok; registered client id {} (server issued a secret: {})",
            client.client_id,
            client.client_secret.is_some(),
        );
        cache_registration(&protected.issuer, redirect_uri, &client);
        client
    };
    Ok(Discovered {
        metadata,
        scopes,
        client,
        resource: protected.resource,
    })
}

/// Maps a discovery failure to the FFI error. Every variant means the same thing to a user;
/// this server does not offer sign-in, use the secret field: so they share one message, with
/// the specific cause kept in the diagnostic log rather than shown.
fn discovery_error(err: mailcal_oauth::DiscoveryError) -> MailcalError {
    log::info!("jmap oauth: discovery unavailable ({err}); the manual secret still works");
    MailcalError::Connect(format!("this server does not support signing in: {err}"))
}
