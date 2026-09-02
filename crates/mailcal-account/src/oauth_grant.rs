//! [`OAuthGrant`]: what an account authenticating by browser sign-in keeps at rest, and
//! the self-refreshing token source built from it.
//!
//! Two account kinds store one of these: a **JMAP** account (`[jmap.oauth]`) and an
//! **IMAP/SMTP** account (`[imap.oauth]`). Nothing here is specific to either, and that is
//! not a coincidence: both are the same standards flow against a server we have never met,
//! discovered rather than integrated (RFC 9728 → RFC 8414 → RFC 7591, see `mailcal-oauth`).
//! A second copy of this type for the second protocol would be two places to forget the
//! same field in, and `resource` has already proved what forgetting one costs.
//!
//! # Why the endpoints are stored rather than re-discovered
//!
//! Discovery runs **once**, at sign-in, and its results are persisted with the account: the
//! endpoints, the registered `client_id`, and the refresh token. A launch therefore costs no
//! discovery round trips and no re-registration, and a server that later withdraws open
//! registration cannot break an account that already has one.
//!
//! # The engine is untouched
//!
//! `provider-jmap` and `provider-imap` both take a finished bearer token and know nothing
//! about OAuth. Refresh lives here, in the provider-neutral [`GraphTokenSource`] the
//! Microsoft and Google accounts use (its name is historical; it is not Graph-specific), and
//! a rotated refresh token is reported to the host's [`TokenSink`] to be re-persisted in the
//! OS keystore.

use std::sync::Arc;

use engine_core::ids::AccountId;
use mailcal_oauth::{AuthStyle, OAuthClient, OAuthProviderConfig};
use serde::Deserialize;

use crate::{AccountError, CredentialOrigin, GraphTokenSource, Secret, TokenSink};

/// The persisted half of an OAuth grant: everything needed to mint a fresh access token at
/// launch without repeating discovery.
///
/// Deserialized from the `[jmap.oauth]` or `[imap.oauth]` section of the account's stored
/// config. Both secrets redact themselves in `Debug`.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthGrant {
    /// The client identifier, from dynamic registration (RFC 7591) at sign-in. Not a secret.
    /// Persisted so a launch never re-registers.
    pub client_id: String,
    /// A client secret, on the rare server that issued one to a client that asked to be
    /// public. Sent on the token exchange and refresh when present. **Not** confidential; an
    /// installed app cannot keep a secret; PKCE is the real protection.
    #[serde(default)]
    pub client_secret: Option<Secret>,
    /// The long-lived refresh token. Rotated in place when the server issues a new one.
    pub refresh_token: Secret,
    /// The discovered authorization endpoint, kept so a re-consent needs no re-discovery.
    pub authorize_endpoint: String,
    /// The discovered token endpoint; where every refresh goes.
    pub token_endpoint: String,
    /// The redirect URI this grant was registered with. Must be replayed verbatim on refresh
    /// and on any re-authorisation, or the server rejects the grant.
    pub redirect_uri: String,
    /// The scopes granted, re-sent on refresh so the grant is re-issued for the same set.
    pub scopes: Vec<String>,
    /// The RFC 8707 resource indicator: the canonical URI of the resource this grant is for,
    /// from the server's RFC 9728 metadata. Persisted because **every refresh must re-send
    /// it**: a server that issues resource-scoped tokens answers `invalid_target` without it,
    /// so dropping it here would break the account at the first refresh rather than at setup.
    /// `None` for a server that publishes no `resource`, and for the IMAP route, which has no
    /// resource URI to name (an IMAP endpoint is not an HTTPS URL and the profile defines no
    /// form for one).
    #[serde(default)]
    pub resource: Option<String>,
    /// The issuer to check a re-authorisation's `iss` against (RFC 9207), when the server
    /// advertised that it sends one. `None` for a server that does not, and for every grant
    /// stored before this field existed: absent means "there was nothing to compare", which
    /// is the pre-RFC-9207 status quo and not a reason to break a working account.
    #[serde(default)]
    pub issuer: Option<String>,
}

impl OAuthGrant {
    /// The `mailcal-oauth` provider config for this grant.
    ///
    /// [`AuthStyle::Discovered`] sends only the parameters RFC 6749 + RFC 7636 define; we
    /// have not read this server's documentation and must not guess at vendor extensions it
    /// may reject.
    #[must_use]
    pub fn provider_config(&self) -> OAuthProviderConfig {
        OAuthProviderConfig {
            authorize_endpoint: self.authorize_endpoint.clone(),
            token_endpoint: self.token_endpoint.clone(),
            client_id: self.client_id.clone(),
            client_secret: self
                .client_secret
                .as_ref()
                .map(|secret| secret.expose().to_owned()),
            redirect_uri: self.redirect_uri.clone(),
            scopes: self.scopes.clone(),
            resource: self.resource.clone(),
            expected_issuer: self.issuer.clone(),
            style: AuthStyle::Discovered,
        }
    }

    /// Serializes this grant as the `oauth` sub-table of an account's config TOML. Manual,
    /// like the rest of the account configs, so the secrets need no `Serialize` impl.
    pub(crate) fn to_table(&self) -> toml::Table {
        let mut table = toml::Table::new();
        table.insert("client_id".into(), self.client_id.clone().into());
        if let Some(secret) = &self.client_secret {
            table.insert("client_secret".into(), secret.expose().to_owned().into());
        }
        table.insert(
            "refresh_token".into(),
            self.refresh_token.expose().to_owned().into(),
        );
        table.insert(
            "authorize_endpoint".into(),
            self.authorize_endpoint.clone().into(),
        );
        table.insert("token_endpoint".into(), self.token_endpoint.clone().into());
        table.insert("redirect_uri".into(), self.redirect_uri.clone().into());
        table.insert(
            "scopes".into(),
            toml::Value::Array(self.scopes.iter().cloned().map(Into::into).collect()),
        );
        if let Some(resource) = &self.resource {
            table.insert("resource".into(), resource.clone().into());
        }
        if let Some(issuer) = &self.issuer {
            table.insert("issuer".into(), issuer.clone().into());
        }
        table
    }
}

/// Builds the shared, self-refreshing token source for `grant`, reusing the same
/// provider-neutral [`GraphTokenSource`] the Microsoft and Google accounts use. `sink`
/// (optional) receives a rotated refresh token for re-persistence in the host's OS keystore.
///
/// `protocol` names the family in the refresh log line (`jmap` / `imap`); a line that cannot
/// say whose token it is answers half the question. It is safe to log: it names the protocol
/// and nothing else.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the OAuth HTTP client cannot be built.
pub fn oauth_token_source(
    grant: &OAuthGrant,
    account: AccountId,
    sink: Option<Arc<dyn TokenSink>>,
    origin: CredentialOrigin,
    protocol: &'static str,
) -> Result<Arc<GraphTokenSource>, AccountError> {
    let oauth = OAuthClient::new(grant.provider_config())
        .map_err(|err| AccountError::Jmap(err.to_string()))?;
    Ok(GraphTokenSource::from_parts(
        oauth,
        account,
        grant.refresh_token.expose().to_owned(),
        sink,
        protocol,
        origin,
    ))
}

#[cfg(test)]
#[path = "oauth_grant_tests.rs"]
mod tests;
