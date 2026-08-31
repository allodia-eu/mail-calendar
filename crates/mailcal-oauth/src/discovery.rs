//! Discovering a protected resource's OAuth authorization server from the standards, so a
//! host can offer "sign in with your provider" for a server it has never heard of.
//!
//! Three RFCs, run in order, none of them provider-specific:
//!
//! 1. **RFC 9728; Protected Resource Metadata.** An unauthenticated request to the resource (for
//!    JMAP, the session URL) is answered `401` with `WWW-Authenticate: Bearer
//!    resource_metadata="…"`. That document names the resource's `authorization_servers`. When the
//!    challenge omits the pointer we fall back to the well-known default location the RFC defines
//!    for the resource URL.
//! 2. **RFC 8414; Authorization Server Metadata.** The issuer's
//!    `/.well-known/oauth-authorization-server` (with the OpenID Connect
//!    `/.well-known/openid-configuration` as the documented fallback) names the
//!    `authorization_endpoint`, `token_endpoint`, optional `registration_endpoint`, the supported
//!    PKCE methods, and the scopes on offer.
//! 3. **RFC 7591; Dynamic Client Registration** ([`crate::register`]), when the metadata advertises
//!    a `registration_endpoint`.
//!
//! # What this refuses to do
//!
//! Discovery is a *trust* decision; it decides which host we will send a user's credentials
//! to: so it is deliberately strict and fails closed. Every hop must be **HTTPS**; the
//! metadata's `issuer` must match the issuer we asked about (RFC 8414 §3.3, which is what
//! stops a compromised resource pointing us at an attacker's token endpoint), and the server
//! must advertise **S256** PKCE, because without it there is no protection on the code
//! exchange for a public client and we would rather fall back to a pasted API token than run
//! a weaker flow. A caller treats *any* [`DiscoveryError`] as "this server does not do
//! discoverable OAuth" and offers the manual secret instead.

use serde::Deserialize;

use crate::OAuthError;

/// The PKCE method we require a server to advertise (RFC 7636 §4.2). `plain` is not
/// acceptable for a public client.
const REQUIRED_PKCE_METHOD: &str = "S256";

/// The RFC 9728 well-known path prefix, used when a `401` challenge carries no
/// `resource_metadata` pointer of its own.
const PROTECTED_RESOURCE_WELL_KNOWN: &str = "/.well-known/oauth-protected-resource";

/// The RFC 8414 well-known path prefix for authorization-server metadata.
const AS_WELL_KNOWN: &str = "/.well-known/oauth-authorization-server";

/// The OpenID Connect Discovery path, the documented fallback when a server publishes its
/// metadata there instead (many do both).
const OIDC_WELL_KNOWN: &str = "/.well-known/openid-configuration";

/// Builds the HTTP client the discovery chain runs on, using the **shared TLS policy** every
/// other network path in the product uses.
///
/// Not a convenience: `reqwest::Client::builder().build()` *panics* in this workspace. The build
/// pins `rustls-no-provider`, so a client constructed without going through
/// [`engine_tls::client_config`] has no crypto provider installed and dies at the first request
/// , and because discovery is a fail-soft path whose errors are deliberately swallowed, that
/// panic surfaces as "this server doesn't support sign-in" rather than as anything diagnosable.
/// Callers must use this rather than rolling their own client.
///
/// # Errors
///
/// Returns [`OAuthError::Tls`] if the shared TLS policy cannot be built, or
/// [`OAuthError::Transport`] if the client cannot be constructed.
pub fn discovery_client() -> Result<reqwest::Client, OAuthError> {
    let tls = engine_tls::client_config(&engine_tls::TlsPolicy::bundled_and_system())
        .map_err(OAuthError::Tls)?;
    tls.reqwest_builder().build().map_err(OAuthError::Transport)
}

/// A failure anywhere in the discovery chain. Every variant means the same thing to a caller
/// (**this server does not offer discoverable OAuth, use the manual secret**) but they are
/// distinguished so a diagnostic log says which step gave up.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// A discovery URL was not HTTPS, or was unparseable. Discovery decides where credentials
    /// are sent, so a plaintext hop is refused outright rather than warned about.
    #[error("oauth discovery: insecure or malformed URL: {0}")]
    InsecureUrl(String),
    /// The resource answered, but not with a `401` bearer challenge naming its metadata, and
    /// no metadata document was found at the well-known location either.
    #[error("oauth discovery: {0} advertises no authorization server")]
    NoAuthorizationServer(String),
    /// A metadata document was fetched but could not be parsed, or omitted a required field.
    #[error("oauth discovery: malformed metadata from {url}: {detail}")]
    MalformedMetadata {
        /// The document's URL.
        url: String,
        /// What was wrong with it.
        detail: String,
    },
    /// The metadata's `issuer` did not match the issuer we asked about (RFC 8414 §3.3); the
    /// check that stops a compromised resource redirecting us to an attacker's endpoints.
    #[error("oauth discovery: issuer mismatch, asked {asked}, document claims {claimed}")]
    IssuerMismatch {
        /// The issuer we requested metadata for.
        asked: String,
        /// The issuer the document claimed to be.
        claimed: String,
    },
    /// The server does not advertise `S256` PKCE, so a public-client code exchange would be
    /// unprotected. We decline rather than run a weaker flow.
    #[error("oauth discovery: {0} does not support S256 PKCE")]
    NoPkce(String),
    /// The network request failed (unreachable, TLS failure, timeout).
    #[error("oauth discovery transport: {0}")]
    Transport(#[source] reqwest::Error),
}

impl From<DiscoveryError> for OAuthError {
    fn from(err: DiscoveryError) -> Self {
        Self::Decode(err.to_string())
    }
}

/// One authorization server's advertised capabilities (RFC 8414), reduced to the fields this
/// client acts on.
#[derive(Debug, Clone)]
pub struct AuthServerMetadata {
    /// The server's canonical issuer identifier, validated against the URL we fetched.
    pub issuer: String,
    /// Where to send the user's browser to authorise.
    pub authorization_endpoint: String,
    /// Where to exchange the code, and later refresh.
    pub token_endpoint: String,
    /// Where to dynamically register a client (RFC 7591), when the server offers it. `None`
    /// means the client id must come from somewhere else: for us, that ends discovery.
    pub registration_endpoint: Option<String>,
    /// Where to revoke a token (RFC 7009), when advertised.
    pub revocation_endpoint: Option<String>,
    /// Where to ask who the token was issued to (OpenID Connect's `userinfo`), when advertised.
    /// `None` for a server that issues no identity: a plain OAuth server protecting mail has no
    /// reason to publish one, and a caller then knows nothing about the person and says nothing.
    pub userinfo_endpoint: Option<String>,
    /// The scopes the server says it supports, used to pick what to request.
    pub scopes_supported: Vec<String>,
    /// Where to end the session the browser holds (OpenID Connect RP-Initiated Logout), when
    /// advertised. Signing out locally does not need it; sending the person there as well is what
    /// stops the next sign-in from completing silently against a session they thought they left.
    pub end_session_endpoint: Option<String>,
    /// The `prompt` values the server accepts (OpenID Connect Prompt Create 1.0 adds `create` to
    /// the four RFC-defined ones). Empty for a server that advertises none, and a caller then
    /// sends no `prompt`: an unadvertised value is a guess, and a server is free to reject the
    /// request outright rather than ignore it.
    pub prompt_values_supported: Vec<String>,
}

/// The raw RFC 8414 document, before validation.
#[derive(Deserialize)]
struct RawAuthServerMetadata {
    issuer: String,
    authorization_endpoint: Option<String>,
    token_endpoint: Option<String>,
    registration_endpoint: Option<String>,
    revocation_endpoint: Option<String>,
    userinfo_endpoint: Option<String>,
    #[serde(default)]
    scopes_supported: Vec<String>,
    #[serde(default)]
    code_challenge_methods_supported: Vec<String>,
    end_session_endpoint: Option<String>,
    #[serde(default)]
    prompt_values_supported: Vec<String>,
}

/// The raw RFC 9728 protected-resource document.
#[derive(Deserialize)]
struct RawResourceMetadata {
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    authorization_servers: Vec<String>,
}

/// What a protected resource says about itself (RFC 9728): which authorization server issues
/// tokens for it, and the **canonical URI naming the resource itself**.
///
/// That second field is not decoration. RFC 8707 lets an authorization server issue tokens
/// scoped to a particular resource, and a server that does so rejects a request that fails to
/// say which one; Fastmail answers `invalid_target`. So the URI discovered here has to ride on
/// the authorization request, the code exchange **and** every refresh.
#[derive(Debug, Clone)]
pub struct ProtectedResource {
    /// The canonical resource URI (the RFC 8707 `resource` parameter value), when the document
    /// names one. `None` for a server that publishes no `resource` field, in which case we send
    /// no target and let the server apply its default.
    pub resource: Option<String>,
    /// The issuer whose metadata describes how to get a token for this resource.
    pub issuer: String,
}

/// Requires `url` to be a syntactically valid HTTPS URL (or a **loopback** one) returning it
/// unchanged.
///
/// This is the single choke point for the "every hop is TLS" rule: a discovery document can
/// name any URL it likes, and following an `http://` one would put an access token, or the
/// authorization code that mints it; on the wire in the clear.
///
/// **Loopback is the one exemption**, and it is exempt because the rule does not apply rather than
/// because it is inconvenient: a request to `127.0.0.1`, `[::1]` or `localhost` never reaches a
/// network, so there is no hop for anyone to read. RFC 8252 §7.3 makes the same allowance for
/// redirect URIs, and every OAuth client makes it for the same reason: an authorization server
/// running on the developer's own machine cannot present a certificate for a name it does not own.
///
/// `localhost` is accepted alongside the literals, with the caveat RFC 8252 §8.3 records: it is a
/// name, so it depends on the resolver. Refusing it would refuse the address such servers actually
/// publish as their issuer, and an attacker who can redirect `localhost` already owns the machine.
///
/// # Errors
///
/// Returns [`DiscoveryError::InsecureUrl`] if `url` does not parse, or is `http` to anywhere but
/// loopback.
fn require_https(url: &str) -> Result<String, DiscoveryError> {
    let parsed =
        url::Url::parse(url).map_err(|err| DiscoveryError::InsecureUrl(format!("{url}: {err}")))?;
    let permitted = match parsed.scheme() {
        "https" => true,
        "http" => is_loopback(&parsed),
        _ => false,
    };
    if !permitted {
        return Err(DiscoveryError::InsecureUrl(url.to_owned()));
    }
    Ok(parsed.into())
}

/// Whether `url`'s host is this machine, and therefore off any network.
///
/// Host-based rather than string-based on purpose: `http://127.0.0.1.example.com/` and
/// `http://evil/?x=localhost` both contain a loopback-looking substring and are neither.
fn is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

/// Builds a well-known metadata URL for `issuer` per RFC 8414 §3.1: the path prefix is
/// inserted **between** the issuer's host and its path, rather than appended, so
/// `https://host/tenant` becomes `https://host/.well-known/…/tenant`, not
/// `https://host/tenant/.well-known/…`. A path-less issuer just gets the prefix.
fn well_known_url(issuer: &url::Url, prefix: &str) -> String {
    let path = issuer.path().trim_end_matches('/');
    let mut url = issuer.clone();
    url.set_path(&format!("{prefix}{path}"));
    url.set_query(None);
    url.set_fragment(None);
    url.into()
}

/// The `resource_metadata` URL a `WWW-Authenticate` header points at, if any.
///
/// The header is a comma-separated list of challenges, each a scheme token optionally
/// followed by `name=value` auth-params (RFC 9110 §11.6.1). We only need the one parameter,
/// so rather than fully parsing the grammar this finds `resource_metadata=` and reads the
/// quoted (or bare) value that follows; commas and other schemes in the header are
/// irrelevant to that.
fn resource_metadata_url(header: &str) -> Option<String> {
    let start = header.find("resource_metadata")?;
    let rest = header[start + "resource_metadata".len()..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    if let Some(quoted) = rest.strip_prefix('"') {
        quoted.find('"').map(|end| quoted[..end].to_owned())
    } else {
        let end = rest.find([',', ' ']).unwrap_or(rest.len());
        Some(rest[..end].to_owned()).filter(|value| !value.is_empty())
    }
}

/// Finds the authorization server protecting `resource_url`, and the resource's own canonical
/// URI (RFC 9728).
///
/// Sends an unauthenticated request and reads the `401`'s `WWW-Authenticate` challenge for a
/// `resource_metadata` pointer; if the challenge carries none, tries the well-known default
/// location for the resource. Returns the **first** advertised issuer; servers list them in
/// preference order, and a client that cannot choose between them should take the first.
///
/// # Errors
///
/// Returns [`DiscoveryError::NoAuthorizationServer`] if neither route yields an issuer,
/// [`DiscoveryError::InsecureUrl`] for a non-HTTPS hop, or a transport/parse error.
pub async fn discover_protected_resource(
    http: &reqwest::Client,
    resource_url: &str,
) -> Result<ProtectedResource, DiscoveryError> {
    let resource_url = require_https(resource_url)?;
    let response = http
        .get(&resource_url)
        .send()
        .await
        .map_err(DiscoveryError::Transport)?;

    // The pointer the challenge gives us, else the RFC's well-known default for this resource.
    let pointer = response
        .headers()
        .get(reqwest::header::WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())
        .and_then(resource_metadata_url);
    let metadata_url = if let Some(url) = pointer {
        require_https(&url)?
    } else {
        let parsed = url::Url::parse(&resource_url)
            .map_err(|err| DiscoveryError::InsecureUrl(format!("{resource_url}: {err}")))?;
        well_known_url(&parsed, PROTECTED_RESOURCE_WELL_KNOWN)
    };

    let metadata: RawResourceMetadata = fetch_json(http, &metadata_url).await?;
    let issuer = metadata
        .authorization_servers
        .into_iter()
        .next()
        .ok_or_else(|| DiscoveryError::NoAuthorizationServer(resource_url))
        .and_then(|issuer| require_https(&issuer))?;
    // The resource URI is sent to the token endpoint, so it is a hop like any other and must be
    // HTTPS; a document naming a plaintext one is malformed rather than merely unusable.
    let resource = metadata
        .resource
        .as_deref()
        .map(require_https)
        .transpose()?;
    Ok(ProtectedResource { resource, issuer })
}

/// Fetches and validates an authorization server's metadata (RFC 8414) for `issuer`.
///
/// Tries the OAuth well-known location, then the OpenID Connect one. The returned document is
/// validated before it is trusted: its `issuer` must match what we asked for, it must name an
/// authorisation **and** token endpoint, and it must advertise `S256` PKCE.
///
/// # Errors
///
/// Returns [`DiscoveryError::IssuerMismatch`], [`DiscoveryError::MalformedMetadata`],
/// [`DiscoveryError::NoPkce`], or a transport error.
pub async fn discover_auth_server(
    http: &reqwest::Client,
    issuer: &str,
) -> Result<AuthServerMetadata, DiscoveryError> {
    let issuer = require_https(issuer)?;
    let parsed = url::Url::parse(&issuer)
        .map_err(|err| DiscoveryError::InsecureUrl(format!("{issuer}: {err}")))?;

    let oauth_url = well_known_url(&parsed, AS_WELL_KNOWN);
    // Many servers publish only the OIDC document; the RFC names it as the fallback.
    let raw = if let Ok(raw) = fetch_json::<RawAuthServerMetadata>(http, &oauth_url).await {
        raw
    } else {
        let oidc_url = well_known_url(&parsed, OIDC_WELL_KNOWN);
        fetch_json::<RawAuthServerMetadata>(http, &oidc_url).await?
    };
    validate(raw, &issuer, &oauth_url)
}

/// Validates a fetched metadata document against the issuer we asked about.
fn validate(
    raw: RawAuthServerMetadata,
    issuer: &str,
    url: &str,
) -> Result<AuthServerMetadata, DiscoveryError> {
    // RFC 8414 §3.3: the document must claim the issuer we asked for. Trailing slashes are not
    // significant in an issuer identifier, so compare without them.
    if raw.issuer.trim_end_matches('/') != issuer.trim_end_matches('/') {
        return Err(DiscoveryError::IssuerMismatch {
            asked: issuer.to_owned(),
            claimed: raw.issuer,
        });
    }
    let missing = |field: &str| DiscoveryError::MalformedMetadata {
        url: url.to_owned(),
        detail: format!("no {field}"),
    };
    let authorization_endpoint = raw
        .authorization_endpoint
        .ok_or_else(|| missing("authorization_endpoint"))?;
    let token_endpoint = raw
        .token_endpoint
        .ok_or_else(|| missing("token_endpoint"))?;
    // Without S256 a public client's code exchange is unprotected; decline rather than
    // silently run a weaker flow the user cannot see.
    if !raw
        .code_challenge_methods_supported
        .iter()
        .any(|method| method == REQUIRED_PKCE_METHOD)
    {
        return Err(DiscoveryError::NoPkce(issuer.to_owned()));
    }
    Ok(AuthServerMetadata {
        issuer: raw.issuer,
        authorization_endpoint: require_https(&authorization_endpoint)?,
        token_endpoint: require_https(&token_endpoint)?,
        registration_endpoint: raw
            .registration_endpoint
            .as_deref()
            .map(require_https)
            .transpose()?,
        revocation_endpoint: raw
            .revocation_endpoint
            .as_deref()
            .map(require_https)
            .transpose()?,
        userinfo_endpoint: raw
            .userinfo_endpoint
            .as_deref()
            .map(require_https)
            .transpose()?,
        scopes_supported: raw.scopes_supported,
        end_session_endpoint: raw
            .end_session_endpoint
            .as_deref()
            .map(require_https)
            .transpose()?,
        prompt_values_supported: raw.prompt_values_supported,
    })
}

/// GETs `url` and decodes its JSON body, requiring a success status.
async fn fetch_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
) -> Result<T, DiscoveryError> {
    let response = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(DiscoveryError::Transport)?;
    let status = response.status();
    let body = response.text().await.map_err(DiscoveryError::Transport)?;
    if !status.is_success() {
        return Err(DiscoveryError::MalformedMetadata {
            url: url.to_owned(),
            detail: format!("http {}", status.as_u16()),
        });
    }
    serde_json::from_str(&body).map_err(|err| DiscoveryError::MalformedMetadata {
        url: url.to_owned(),
        detail: err.to_string(),
    })
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;
