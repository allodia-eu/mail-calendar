//! Signing in to an Allodia account.
//!
//! Thin on purpose. The whole OAuth flow (RFC 8414 discovery, PKCE, the code exchange, refresh
//! rotation) is `mailcal-oauth`'s, already exercised against Microsoft, Google and
//! standards-discovered JMAP. What is Allodia's here is four facts: which issuer, which scopes,
//! which redirect, and that the client registration is injected rather than self-registered.
//!
//! The registration is **static**, not RFC 7591 self-registered: a first-party client has no reason
//! to mint a fresh one on every install, and a static one can be revoked.

use mailcal_oauth::{
    AuthRequest, AuthStyle, OAuthClient, OAuthError, OAuthProviderConfig, TokenSet, credentials,
    discover_auth_server, discover_protected_resource, discovery_client,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// The account service this build talks to.
///
/// One address, chosen at build time and with no runtime path to it, which is what the
/// sovereignty carve-out in `entitlement.md` rests on. A development build points at a local
/// instance; everything else gets [`credentials::DEFAULT_ALLODIA_HOST`].
///
/// It is the **only** thing about the service this build knows. The audience a token must carry
/// and the authorization server that mints it are both read from the API's own RFC 9728 metadata,
/// so neither can drift from what the service actually verifies.
#[must_use]
pub fn host() -> String {
    credentials::allodia_host()
}

/// What sign-in asks for, and every one is a scope the service advertises.
///
/// `openid`, `profile` and `email` identify the person, so a client can say which account is signed
/// in. **`offline_access` is the load-bearing one**: without it the service issues no refresh
/// token, and the sign-in silently becomes a session that expires with no way back, which is the
/// whole problem OAuth was chosen to solve here.
///
/// `mailcal:entitlement:read` is what the entitlement endpoint requires, and it is the narrowest
/// thing this app needs: permission to read which plan an account is on, and nothing else. Nothing
/// here reaches mail: an Allodia account and a mail account are different things, and a token
/// issued for this app cannot touch the second.
pub const SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "mailcal:entitlement:read",
    "mailcal:accounts:read",
    "mailcal:accounts:write",
];

/// The ones a sign-in is not worth completing without, sent whether or not the service lists them.
///
/// `offline_access` is the load-bearing one and `openid`/`profile`/`email` are how the app learns
/// whose account it is. Filtering these against an incomplete `scopes_supported` would turn a
/// service that simply under-advertises into a sign-in that succeeds and then cannot say who
/// signed in, or one that expires within the hour with no way back.
const REQUIRED_SCOPES: &[&str] = &["openid", "profile", "email", "offline_access"];

/// What a build asks for **only** where the service says it accepts it.
///
/// These gate features rather than the sign-in itself, so a client that reaches a deployment
/// predating them should lose the feature and keep the sign-in. Asking for a scope a server has
/// not advertised is refused outright by enough of them that the alternative is a client which
/// cannot sign in at all until the server catches up.
fn optional_scopes(advertised: &[String]) -> Vec<String> {
    SCOPES
        .iter()
        .filter(|scope| !REQUIRED_SCOPES.contains(*scope))
        .filter(|scope| advertised.iter().any(|offered| offered == *scope))
        .map(|scope| (*scope).to_owned())
        .collect()
}

/// Everything to ask for, given what the service says it accepts.
pub(crate) fn scopes_for(advertised: &[String]) -> Vec<String> {
    let mut scopes: Vec<String> = REQUIRED_SCOPES.iter().map(|s| (*s).to_owned()).collect();
    scopes.extend(optional_scopes(advertised));
    scopes
}

/// The host component of the custom-scheme redirect: `<application-id>://account-oauth`.
///
/// Stated once here because four clients have to agree on it and a typo in one is a platform that
/// silently never comes back from the browser. Windows and Android dispatch a callback on this
/// label, so it has to differ from the ones already in use (`auth` for Microsoft, `jmap-oauth` for
/// JMAP). The scheme itself is the injected application id, so Allodia's builds redirect to
/// `eu.allodia.mailcal://account-oauth` and an unbranded build would use its own id, which needs
/// registering nowhere, because such a build carries no registration and offers no sign-in.
///
/// Linux is the exception and uses a loopback listener on an ephemeral port instead, as it does for
/// Microsoft: the app claims no URI scheme on that platform.
pub const REDIRECT_HOST: &str = "account-oauth";

/// The API this sign-in is for: the URL whose own metadata names its audience and its
/// authorization server.
#[must_use]
pub fn api_url() -> String {
    format!("{}{}", host(), crate::API_BASE_PATH)
}

/// The page a person manages their own account on, including deleting it.
///
/// Derived from [`host()`] rather than discovered, because it is a product page and no OAuth
/// metadata describes one, and derived rather than written out, so a development build pointed at
/// a local service reaches that service's page and not the production one.
#[must_use]
pub fn account_url() -> String {
    format!("{}{}", host(), ACCOUNT_PATH)
}

/// Where [`account_url()`] points, relative to the service's root.
const ACCOUNT_PATH: &str = "/account";

/// Why a sign-in could not start or finish.
#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    /// This build carries no Allodia client registration, so it offers no Allodia sign-in. Not a
    /// failure: it is what every build from source is, and a caller checks [`available`] first.
    #[error("this build carries no Allodia client registration")]
    Unavailable,
    /// The service's OAuth metadata could not be read.
    #[error("the account service's sign-in metadata could not be read: {0}")]
    Discovery(#[from] mailcal_oauth::DiscoveryError),
    /// The flow itself failed.
    #[error(transparent)]
    OAuth(#[from] OAuthError),
    /// The service would not say who the token belongs to, so there is no account to name.
    #[error("the account service did not say whose account this is: {0}")]
    NoIdentity(String),
}

/// Whether this build can offer Allodia sign-in at all.
///
/// A client asks before it draws the button. False is the ordinary answer for a build from source,
/// and the surface is then absent rather than present-and-broken: the same rule every other
/// provider follows.
#[must_use]
pub fn available() -> bool {
    credentials::allodia_client_id().is_some()
}

/// Who signed in, for a client to show beside the sign-out button.
///
/// The address is what identifies the account; the name is a courtesy the service may not hold.
/// Neither decides anything: what an account may *do* is the entitlement's answer, and that one is
/// resolved from the token by the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// The account's email address.
    pub email: String,
    /// The person's display name, when the service holds one.
    pub name: Option<String>,
}

/// The OpenID Connect `userinfo` claims this reads. Everything else the document carries is
/// ignored rather than rejected, so a service that adds a claim does not break a client.
#[derive(serde::Deserialize)]
struct UserinfoClaims {
    email: Option<String>,
    name: Option<String>,
}

/// What discovery found, in a form that survives the browser round trip.
///
/// A sign-in is two calls with a person in between, and the second one has to reach the *same*
/// token endpoint as the first: an authorization code is minted for one server and is worthless
/// anywhere else. Re-running discovery to find it again would put two more requests -- and two more
/// ways to fail -- after the code has already been issued and cannot be re-obtained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoints {
    /// Where the browser was sent.
    pub authorize_endpoint: String,
    /// Where the code is exchanged, and where every later refresh goes.
    pub token_endpoint: String,
    /// The RFC 8707 resource indicator the authorization request carried. It has to ride on the
    /// exchange as well: a token minted for a different audience is refused by the API and by
    /// nothing before it.
    pub resource: Option<String>,
    /// Where to ask who the token belongs to, when the server advertises it.
    pub userinfo_endpoint: Option<String>,
    /// Where to end the browser session (OpenID Connect RP-Initiated Logout), when the server
    /// advertises it. Carried so a sign-out can hand the URL back without a discovery round trip
    /// it would have to make *after* deciding to sign out.
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    /// The `prompt` values the server accepts. Carried for the same reason, and read before
    /// anything sends one.
    #[serde(default)]
    pub prompt_values_supported: Vec<String>,
    /// The scopes this flow actually asked for, after intersecting what this build wants with what
    /// the service advertises. Carried so the exchange and every later refresh name the same set
    /// the authorization request did.
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Which first step the person needs.
///
/// Two, because someone who has no account and someone returning to one want different pages, and
/// guessing wrong costs a round trip through a form they did not want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// The ordinary sign-in page.
    SignIn,
    /// The registration page, as OpenID Connect Prompt Create 1.0 defines it.
    Create,
}

/// The `prompt` value Prompt Create 1.0 defines for registration.
const PROMPT_CREATE: &str = "create";

/// Read the service's OAuth metadata and build a client against it.
///
/// Shared by the two things that need one, so the decisions below are made once: a sign-in, which
/// reads the identity and logout halves of the metadata as well, and a
/// [`Refresher`](crate::Refresher), which needs the client and nothing else.
///
/// `redirect_uri` is only ever put on an authorization request, so a caller that will only refresh
/// has none to give (RFC 6749 §6) and passes an empty one.
pub(crate) async fn discovered_client(
    redirect_uri: &str,
) -> Result<(OAuthClient, mailcal_oauth::AuthServerMetadata), SignInError> {
    let client_id = credentials::allodia_client_id().ok_or(SignInError::Unavailable)?;
    let http = discovery_client()?;
    let resource = discover_protected_resource(&http, &api_url()).await?;
    let metadata = discover_auth_server(&http, &resource.issuer).await?;
    let client = OAuthClient::new(OAuthProviderConfig {
        authorize_endpoint: metadata.authorization_endpoint.clone(),
        token_endpoint: metadata.token_endpoint.clone(),
        client_id,
        // No secret. The account service is ours and issues public clients; the Google Desktop
        // secret exists because Google's token endpoint demands one, and nothing here does.
        client_secret: None,
        redirect_uri: redirect_uri.to_owned(),
        scopes: scopes_for(&metadata.scopes_supported),
        // RFC 8707, and the API's own canonical URI rather than one built here. The service mints
        // a verifiable JWT for a named resource and an opaque token without one, and the API
        // refuses anything not minted for itself -- so this is not optional, and omitting it fails
        // as a `401` that names neither cause.
        resource: resource.resource.or_else(|| Some(api_url())),
        // Discovered, not integrated: send only what RFC 6749 and RFC 7636 define. Nothing here
        // may guess at an extension the server has not advertised.
        style: AuthStyle::Discovered,
    })?;
    Ok((client, metadata))
}

/// A sign-in flow bound to this build's registration.
#[derive(Debug)]
pub struct SignIn {
    client: OAuthClient,
    /// Where to ask who a token belongs to, as the authorization server's own metadata named it.
    /// `None` for a server that publishes none, which Allodia's does not do, and which is
    /// therefore reported rather than papered over.
    userinfo_endpoint: Option<String>,
    /// Where to end the browser session, when the server advertises one.
    end_session_endpoint: Option<String>,
    /// What the server accepts as a `prompt`. Nothing is sent that is not in here.
    prompt_values_supported: Vec<String>,
}

/// Whether this request carries `prompt=create`.
///
/// Both halves matter. Asking for a sign-in never sends one, and asking to create sends one only
/// where the server advertised it: an unadvertised `prompt` is a guess, and a server is free to
/// refuse the whole request rather than ignore the parameter.
fn sends_create(prompt: Prompt, supported: &[String]) -> bool {
    prompt == Prompt::Create && supported.iter().any(|value| value == PROMPT_CREATE)
}

/// Append `prompt=create` to an authorization URL that has query parameters already.
///
/// The URL comes from `OAuthClient::begin`, which always writes at least `client_id`,
/// `response_type` and the PKCE challenge, so the separator is always `&`. Percent-encoding is not
/// in question: the value is one lowercase word this file owns.
fn append_prompt_create(authorization_url: &str) -> String {
    debug_assert!(authorization_url.contains('?'));
    format!("{authorization_url}&prompt={PROMPT_CREATE}")
}

#[cfg(test)]
#[path = "signin_tests.rs"]
mod signin_tests;

impl SignIn {
    /// Discover the API's audience and its authorization server, then build the client.
    ///
    /// Two hops, both the standards': RFC 9728 asks the API which server protects it and what its
    /// canonical URI is, then RFC 8414 asks that server for its endpoints. Nothing about either is
    /// assumed: the same chain a discovered JMAP server goes through.
    ///
    /// # Errors
    /// [`SignInError::Unavailable`] when the build carries no registration;
    /// [`SignInError::Discovery`] when either document cannot be read.
    pub async fn discover(redirect_uri: &str) -> Result<Self, SignInError> {
        let (client, metadata) = discovered_client(redirect_uri).await?;
        Ok(Self {
            client,
            userinfo_endpoint: metadata.userinfo_endpoint,
            end_session_endpoint: metadata.end_session_endpoint,
            prompt_values_supported: metadata.prompt_values_supported,
        })
    }

    /// Rebuild a flow from what a previous [`SignIn::discover`] found, without asking again.
    ///
    /// # Errors
    /// [`SignInError::Unavailable`] when the build carries no registration; [`SignInError::OAuth`]
    /// when the endpoints do not make a usable client.
    pub fn resume(redirect_uri: &str, endpoints: Endpoints) -> Result<Self, SignInError> {
        let client_id = credentials::allodia_client_id().ok_or(SignInError::Unavailable)?;
        let client = OAuthClient::new(OAuthProviderConfig {
            authorize_endpoint: endpoints.authorize_endpoint,
            token_endpoint: endpoints.token_endpoint,
            client_id,
            client_secret: None,
            redirect_uri: redirect_uri.to_owned(),
            // What the first half asked for. An older handle carries none, and falls back to
            // the full set, which is what that build sent anyway.
            scopes: if endpoints.scopes.is_empty() {
                SCOPES.iter().map(|scope| (*scope).to_owned()).collect()
            } else {
                endpoints.scopes.clone()
            },
            resource: endpoints.resource,
            style: AuthStyle::Discovered,
        })?;
        Ok(Self {
            client,
            userinfo_endpoint: endpoints.userinfo_endpoint,
            end_session_endpoint: endpoints.end_session_endpoint,
            prompt_values_supported: endpoints.prompt_values_supported,
        })
    }

    /// What this flow discovered, for a caller to hold across the browser round trip and hand back
    /// to [`SignIn::resume`].
    #[must_use]
    pub fn endpoints(&self) -> Endpoints {
        let provider = self.client.provider();
        Endpoints {
            authorize_endpoint: provider.authorize_endpoint.clone(),
            token_endpoint: provider.token_endpoint.clone(),
            resource: provider.resource.clone(),
            userinfo_endpoint: self.userinfo_endpoint.clone(),
            end_session_endpoint: self.end_session_endpoint.clone(),
            prompt_values_supported: self.prompt_values_supported.clone(),
            scopes: provider.scopes.clone(),
        }
    }

    /// Start a flow. The caller opens [`AuthRequest::authorization_url`] in the platform's in-app
    /// browser tab and keeps the `state` and PKCE pair for [`SignIn::complete`].
    ///
    /// [`Prompt::Create`] asks for the registration page instead of the sign-in one, and is sent
    /// **only** when the server advertises `create` in `prompt_values_supported`. A server that
    /// does not gets an ordinary sign-in request: its page is where someone registers anyway, so
    /// the fallback costs a click rather than the flow, and sending an unadvertised parameter
    /// risks the request being refused outright.
    #[must_use]
    pub fn begin(&self, prompt: Prompt) -> AuthRequest {
        // No login hint: an Allodia account is not a mail address the app already knows, and
        // pre-filling one would guess.
        let request = self.client.begin(None);
        if sends_create(prompt, &self.prompt_values_supported) {
            return AuthRequest {
                authorization_url: append_prompt_create(&request.authorization_url),
                ..request
            };
        }
        request
    }

    /// Whether asking for [`Prompt::Create`] would actually reach the registration page.
    ///
    /// A client reads this to decide whether the two routes are worth drawing separately.
    #[must_use]
    pub fn supports_create(&self) -> bool {
        sends_create(Prompt::Create, &self.prompt_values_supported)
    }

    /// Where to send the browser to end its own session, when the server advertises it.
    ///
    /// ⚠️ This does **not** end this build's grant. It closes the browser session, and the tokens
    /// bound to it, but a refresh token carrying `offline_access` is preserved on purpose, and
    /// this build requests `offline_access`. Erasing the local grant is what signs this device
    /// out; this is what stops the next sign-in completing silently against a session the person
    /// thought they had left.
    #[must_use]
    pub fn end_session_url(&self) -> Option<String> {
        self.end_session_endpoint.clone()
    }

    /// What this flow asked the service for, after intersecting what the build wants with what
    /// the service advertises.
    ///
    /// The caller stores it beside the grant, because a token response names the granted scope
    /// only when it *differs* from the request (RFC 6749 §5.1), so on the common path the
    /// request is the only record of what was issued.
    #[must_use]
    pub fn requested_scopes(&self) -> &[String] {
        self.client.requested_scopes()
    }

    /// Finish a flow from the redirect the browser handed back. The verifier is
    /// [`AuthRequest::pkce`]'s, held by the caller across the browser round trip.
    ///
    /// # Errors
    /// [`SignInError::OAuth`] when the callback is malformed, its `state` does not match, or the
    /// exchange is refused.
    pub async fn complete(
        &self,
        callback_url: &str,
        expected_state: &str,
        pkce_verifier: &str,
        now: OffsetDateTime,
    ) -> Result<TokenSet, SignInError> {
        Ok(self
            .client
            .complete(callback_url, expected_state, pkce_verifier, now)
            .await?)
    }

    /// Ask the service whose account a token belongs to.
    ///
    /// The `openid`, `profile` and `email` scopes are requested for exactly this, and it is the
    /// only reason they are: a client has to be able to say *which* account is signed in, and a
    /// person with two Allodia accounts is otherwise shown the same screen for both.
    ///
    /// # Errors
    /// [`SignInError::NoIdentity`] when the server publishes no `userinfo` endpoint, refuses the
    /// token, or answers without an address.
    pub async fn identity(&self, access_token: &str) -> Result<Identity, SignInError> {
        let endpoint = self.userinfo_endpoint.as_deref().ok_or_else(|| {
            SignInError::NoIdentity("it advertises no userinfo endpoint".to_owned())
        })?;
        let http = discovery_client()?;
        let response = http
            .get(endpoint)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|error| SignInError::NoIdentity(error.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| SignInError::NoIdentity(error.to_string()))?;
        if !status.is_success() {
            // The body is the server's own words about a token it just refused, and it is the
            // difference between a scope that was not granted and a token minted for the wrong
            // audience. Neither is visible from the status alone.
            return Err(SignInError::NoIdentity(format!("{status}: {body}")));
        }
        let claims: UserinfoClaims = serde_json::from_str(&body)
            .map_err(|error| SignInError::NoIdentity(error.to_string()))?;
        let email = claims
            .email
            .filter(|email| !email.is_empty())
            .ok_or_else(|| SignInError::NoIdentity("it named no address".to_owned()))?;
        Ok(Identity {
            email,
            name: claims.name.filter(|name| !name.is_empty()),
        })
    }

    /// Exchange a refresh token for a fresh access token.
    ///
    /// # Errors
    /// [`SignInError::OAuth`] when the grant has been revoked or the service refuses it. A caller
    /// treats that as signed-out, not as an outage: the entitlement cache already covers outages.
    pub async fn refresh(
        &self,
        refresh_token: &str,
        now: OffsetDateTime,
    ) -> Result<TokenSet, SignInError> {
        Ok(self.client.refresh(refresh_token, now).await?)
    }
}
