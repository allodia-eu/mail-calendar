//! The per-provider OAuth endpoints + client registration, and the authorisation-URL
//! builder.
//!
//! [`OAuthProviderConfig`] is provider-agnostic: a URL pair, a public-client id, a
//! redirect URI, and the requested scopes; with a [`OAuthProviderConfig::microsoft`]
//! constructor for the Microsoft identity platform (v2.0 endpoints). Gmail and any
//! other Authorization-Code+PKCE provider are just different endpoint/scope values, so
//! they reuse everything here.

/// The Microsoft identity-platform authority host (worldwide/public cloud).
const MS_AUTHORITY: &str = "https://login.microsoftonline.com";

/// The delegated Graph scopes a Microsoft account requests:
/// `offline_access` (to be issued a refresh token at all), the OIDC scopes that name the
/// signed-in user, `Mail.ReadWrite` for the Graph mail sync **and** the write actions
/// (mark-read/flag, move/archive, permanent delete), `Mail.Send` for submission
/// (`POST /me/sendMail`: a distinct scope; `Mail.ReadWrite` does **not** grant send),
/// `Calendars.ReadWrite` for the calendar read/sync + create/patch/delete, `User.Read`
/// so the core can look up the account's own address (`GET /me`) to name it,
/// `Contacts.ReadWrite` for the account's saved contacts, and `User.ReadBasic.All` for
/// the tenant directory: the people a work/school account actually corresponds with,
/// and the permission a colleague's profile photo is read through.
///
/// Scopes are granted by **incremental consent**: an account whose stored grant predates a
/// scope 403s (`ErrorAccessDenied`) on that capability until it re-authenticates: a reconnect
/// re-requests this whole set, so re-consenting for any one scope grants them all. So widening
/// what is *requested* here makes every existing account re-authenticate before the new
/// capability works. Each scope must also be a delegated permission on the Azure app
/// registration or consent fails.
///
/// **A scope the account cannot grant fails at *connect*, not at use.** Microsoft answers
/// `access_denied` during consent, so an unregistered (or admin-gated) scope does not cost one
/// capability, it stops the account being added at all
/// ([`docs/provider-oauth.md`](../../../docs/provider-oauth.md) rule 10). That is why this set
/// stays to permissions a *user* can consent to for themselves, and why two Graph contact
/// permissions are deliberately absent: `ProfilePhoto.Read.All`, which grants nothing
/// `User.ReadBasic.All` does not (verified against a real tenant), and `OrgContact.Read.All`,
/// which covers a source the product does not read. Both are tenant-wide reads; requesting
/// either would put every user in a tenant that requires admin approval behind their
/// administrator before they could connect an account.
///
/// **Contacts are requested read *and write* while the product only reads them.** The
/// alternative is asking every Microsoft user to re-consent a second time the moment contact
/// editing ships, and a forced re-authentication is a worse experience than one broader prompt
/// now. What the app actually does is bounded by
/// [`docs/privacy-policy.md`](../../../docs/privacy-policy.md), which states the read-only
/// behaviour plainly and explains the gap: the promise is kept by the policy and the code, not
/// by the narrowness of the scope. Revisit if contact editing is dropped.
pub const MICROSOFT_GRAPH_SCOPES: &[&str] = &[
    "offline_access",
    "openid",
    "profile",
    "email",
    "https://graph.microsoft.com/Mail.ReadWrite",
    "https://graph.microsoft.com/Mail.Send",
    "https://graph.microsoft.com/Calendars.ReadWrite",
    "https://graph.microsoft.com/User.Read",
    "https://graph.microsoft.com/Contacts.ReadWrite",
    "https://graph.microsoft.com/User.ReadBasic.All",
];

/// The Google identity-platform OAuth 2.0 endpoints; fixed (Google has no per-tenant
/// authority), shared by Gmail and Google Calendar.
const GOOGLE_AUTHORIZE_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// The delegated scopes a Google account requests: **full** Gmail (`mail.google.com`) and
/// read/write Google Calendar.
///
/// The engine's Gmail provider does the full range of mail writes; `messages.modify`/`trash`
/// **and permanent `messages.delete`** plus `messages.send`, and permanent delete is only
/// granted by the broad `https://mail.google.com/` scope, so that is what we request rather
/// than composing narrower `gmail.*` scopes. The account's own address is read from the Gmail
/// `users/me/profile` endpoint (covered by this scope), so no `openid`/`email` scope is needed.
///
/// The three contact scopes cover the same ground Microsoft's do: `contacts` for the user's own
/// address book, `contacts.other.readonly` for the addresses Google collects from mail on their
/// behalf, and `directory.readonly` for colleagues on a Workspace domain. As with Microsoft,
/// `contacts` is requested read **and write** although this release only reads; widening later
/// would force every Google account through a second consent, and the read-only promise lives in
/// [`docs/privacy-policy.md`](../../../docs/privacy-policy.md) rather than in the scope.
///
/// Unlike Microsoft's `offline_access`, Google issues a refresh token from the request
/// **parameters** `access_type=offline` + `prompt=consent` (see [`AuthStyle::Google`]), not a
/// scope.
///
/// **Two verification tiers are in play, and only the mail one is expensive.** `mail.google.com`
/// and `calendar` are **restricted** scopes: the app stays unverified; usable only by
/// allow-listed Early Access test users; until Google's security assessment clears. The three
/// contact scopes are **sensitive**, not restricted, which is a declaration, a justification and
/// a demo video rather than a second assessment, so they do not deepen the gate the app is
/// already behind.
pub const GOOGLE_SCOPES: &[&str] = &[
    "https://mail.google.com/",
    "https://www.googleapis.com/auth/calendar",
    "https://www.googleapis.com/auth/contacts",
    "https://www.googleapis.com/auth/contacts.other.readonly",
    "https://www.googleapis.com/auth/directory.readonly",
];

/// How a provider wants its authorization request shaped, beyond the shared
/// PKCE/state/scope/redirect params: the two integrated providers differ in how a refresh
/// token is issued and how a known account is targeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// Microsoft identity platform: `response_mode=query`; a `login_hint` targets the account
    /// and suppresses the picker, and with **no** hint `prompt=select_account` shows it. A
    /// refresh token comes from the `offline_access` scope.
    Microsoft,
    /// Google: `access_type=offline` **and** `prompt=consent` on **every** request, so Google
    /// issues a refresh token even on a repeat authorisation (it omits one otherwise, which
    /// would break the account an hour later); a `login_hint` pre-fills the account.
    Google,
    /// A server discovered from its own metadata (RFC 8414) rather than integrated by hand,
    /// today, a JMAP provider. Sends **only** the parameters RFC 6749 + RFC 7636 define, plus
    /// a `login_hint` when the address is known. Nothing vendor-specific: we have not read
    /// this server's documentation and must not guess at extensions it may not accept. A
    /// refresh token comes from the `offline_access` scope, which discovery always requests.
    Discovered,
}

/// One OAuth provider's endpoints and this app's client registration.
///
/// Always a **public client**: PKCE (the S256 code verifier) is what actually protects the token
/// exchange, and an installed desktop/mobile app cannot keep a real secret. Most client types we
/// integrate therefore send **no** `client_secret` at all (Microsoft; Google's iOS/Android
/// clients). The one exception is a **Google _Desktop_ client** (the macOS/Windows loopback
/// flow): Google's token endpoint rejects even a valid PKCE exchange with
/// `invalid_request; client_secret is missing` unless the (embedded, **non-confidential**)
/// Desktop secret is present. That value is carried in [`OAuthProviderConfig::client_secret`];
/// Google documents it as *not* treated as a secret. See
/// <https://developers.google.com/identity/protocols/oauth2#installed>.
#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    /// The authorization endpoint (front channel; opened in the system browser).
    pub authorize_endpoint: String,
    /// The token endpoint (back channel: the code→token and refresh POSTs).
    pub token_endpoint: String,
    /// The registered application (client) id. Not a secret.
    pub client_id: String,
    /// An optional, **non-confidential** client secret sent on the token exchange (both the code
    /// exchange and the refresh).
    ///
    /// `None` for every true public client (Microsoft, and Google's iOS/Android clients) whose
    /// token endpoint needs no secret and where PKCE stands alone. `Some` **only** for a Google
    /// _Desktop_ client (the macOS/Windows loopback flow): Google's token endpoint rejects the
    /// PKCE exchange with `invalid_request; client_secret is missing` without it, even though the
    /// client is public. Google documents this secret as *not* confidential; it is embedded in
    /// the app's source and PKCE remains the real protection. See
    /// <https://developers.google.com/identity/protocols/oauth2#installed>.
    pub client_secret: Option<String>,
    /// The registered redirect URI the browser returns to (a custom scheme, e.g.
    /// `eu.allodia.mailcal://oauth`, captured by the platform's auth session, or a
    /// `http://127.0.0.1:<port>/…` loopback for the Google Desktop flow).
    pub redirect_uri: String,
    /// The delegated scopes to request (space-joined into the request).
    pub scopes: Vec<String>,
    /// The RFC 8707 `resource`: the canonical URI of the protected resource this token is for,
    /// discovered from the resource's own RFC 9728 metadata.
    ///
    /// `None` for the integrated providers, which scope tokens by scope alone. `Some` for a
    /// discovered JMAP server, where it is **required**: a server that issues resource-scoped
    /// tokens rejects a request that does not name its target (Fastmail answers `invalid_target`).
    /// It must ride on the authorization request, the code exchange, and every refresh; omitting
    /// it from the refresh alone would break the account about an hour after setup.
    pub resource: Option<String>,
    /// How to shape the authorization request (provider-specific params + account targeting).
    pub style: AuthStyle,
}

impl OAuthProviderConfig {
    /// Builds a Microsoft identity-platform (v2.0) config for `tenant`; typically
    /// `common` (both work and personal accounts), or `organizations` / `consumers` /
    /// a specific tenant id. The host passes the `client_id` and `redirect_uri` from
    /// its registered Azure app; `scopes` is usually [`MICROSOFT_GRAPH_SCOPES`].
    #[must_use]
    pub fn microsoft(
        client_id: impl Into<String>,
        tenant: &str,
        redirect_uri: impl Into<String>,
        scopes: &[&str],
    ) -> Self {
        let tenant = if tenant.trim().is_empty() {
            "common"
        } else {
            tenant.trim()
        };
        Self {
            authorize_endpoint: format!("{MS_AUTHORITY}/{tenant}/oauth2/v2.0/authorize"),
            token_endpoint: format!("{MS_AUTHORITY}/{tenant}/oauth2/v2.0/token"),
            client_id: client_id.into(),
            // Microsoft is a true public client: no secret, PKCE alone.
            client_secret: None,
            redirect_uri: redirect_uri.into(),
            scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
            // Microsoft scopes tokens by scope, not by RFC 8707 resource indicator.
            resource: None,
            style: AuthStyle::Microsoft,
        }
    }

    /// Builds a Google OAuth 2.0 config. The host passes the `client_id` and `redirect_uri`
    /// from its registered Google Cloud OAuth client, and `scopes` is usually [`GOOGLE_SCOPES`].
    ///
    /// Every Google client type we use is a **public client**; PKCE secures the exchange. Pass
    /// `client_secret = None` for the iOS/Android clients (their token endpoint needs no secret).
    /// Pass `Some(secret)` **only** for a Google _Desktop_ client (the macOS/Windows loopback
    /// flow), whose token endpoint requires the embedded, **non-confidential** Desktop secret even
    /// under PKCE; see [`OAuthProviderConfig::client_secret`] and
    /// <https://developers.google.com/identity/protocols/oauth2#installed>.
    #[must_use]
    pub fn google(
        client_id: impl Into<String>,
        client_secret: Option<String>,
        redirect_uri: impl Into<String>,
        scopes: &[&str],
    ) -> Self {
        Self {
            authorize_endpoint: GOOGLE_AUTHORIZE_ENDPOINT.to_owned(),
            token_endpoint: GOOGLE_TOKEN_ENDPOINT.to_owned(),
            client_id: client_id.into(),
            client_secret,
            redirect_uri: redirect_uri.into(),
            scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
            // Google likewise uses scopes alone.
            resource: None,
            style: AuthStyle::Google,
        }
    }

    /// Builds the front-channel authorization URL to open in the system browser:
    /// an Authorization-Code request carrying the PKCE `challenge` (S256) and the CSRF
    /// `state`. When a `login_hint` (the address the user is connecting) is given, it is
    /// passed through so the provider pre-fills and targets that account: the app knows
    /// which account this is for, so it does not make the user pick.
    ///
    /// The remaining, provider-specific params come from [`AuthStyle`]: Microsoft adds
    /// `response_mode=query` and, with no hint, `prompt=select_account`; Google always adds
    /// `access_type=offline` + `prompt=consent` so a refresh token is issued every time.
    #[must_use]
    pub fn authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
        login_hint: Option<&str>,
    ) -> String {
        let mut url =
            url::Url::parse(&self.authorize_endpoint).expect("authorize endpoint is a valid URL");
        let hint = login_hint.map(str::trim).filter(|hint| !hint.is_empty());
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("client_id", &self.client_id)
                .append_pair("response_type", "code")
                .append_pair("redirect_uri", &self.redirect_uri)
                .append_pair("scope", &self.scopes.join(" "))
                .append_pair("state", state)
                .append_pair("code_challenge", code_challenge)
                .append_pair("code_challenge_method", "S256");
            // RFC 8707: name the resource the token is for. Sent on the authorization request as
            // well as the exchange, because a server may bind the issued code to the target.
            if let Some(resource) = &self.resource {
                pairs.append_pair("resource", resource);
            }
            match self.style {
                AuthStyle::Microsoft => {
                    pairs.append_pair("response_mode", "query");
                    match hint {
                        Some(hint) => {
                            pairs.append_pair("login_hint", hint);
                        }
                        None => {
                            pairs.append_pair("prompt", "select_account");
                        }
                    }
                }
                AuthStyle::Discovered => {
                    if let Some(hint) = hint {
                        pairs.append_pair("login_hint", hint);
                    }
                }
                AuthStyle::Google => {
                    // `access_type=offline` + `prompt=consent` guarantee a refresh token on
                    // every authorisation; Google returns one only on the first grant
                    // otherwise, and a re-consent (e.g. after a revoked grant) would then get
                    // no refresh token and the account would break an hour later.
                    pairs
                        .append_pair("access_type", "offline")
                        .append_pair("prompt", "consent");
                    if let Some(hint) = hint {
                        pairs.append_pair("login_hint", hint);
                    }
                }
            }
        }
        url.into()
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
