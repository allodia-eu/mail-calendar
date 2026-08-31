//! The OAuth grant a JMAP account may authenticate with, and the self-refreshing token
//! source built from it.
//!
//! A JMAP account has two ways to authenticate, and this is the second one. The first; a
//! password or API token the user pasted, is a single stored secret and needs nothing here.
//! This one is a full Authorization-Code + PKCE grant against an authorization server
//! **discovered from the standards** (RFC 9728 → RFC 8414 → RFC 7591; see `mailcal-oauth`),
//! so the user signs in with their provider instead of minting a token by hand.
//!
//! # Why the endpoints are stored rather than re-discovered
//!
//! Discovery runs **once**, at sign-in, and its results are persisted with the account: the
//! endpoints, the registered `client_id`, and the refresh token. A launch therefore costs no
//! discovery round trips and no re-registration: the brief's "persist the DCR client id per
//! install", and a server that later withdraws open registration cannot break an account
//! that already has one.
//!
//! # The engine is untouched
//!
//! `provider-jmap` takes a finished bearer token and knows nothing about OAuth. Refresh lives
//! here, in the same provider-neutral [`GraphTokenSource`] the Microsoft and Google accounts
//! use (its name is historical; it is not Graph-specific), and a rotated refresh token is
//! reported to the host's [`TokenSink`] to be re-persisted in the OS keystore.
//!
//! [`TokenSink`]: crate::TokenSink

use std::sync::Arc;

use engine_core::ids::AccountId;
use mailcal_oauth::{AuthStyle, OAuthClient, OAuthProviderConfig};
use serde::Deserialize;

use crate::{AccountError, GraphTokenSource, Secret, TokenSink};

/// The persisted half of a JMAP OAuth grant: everything needed to mint a fresh access token
/// at launch without repeating discovery.
///
/// Deserialized from the `[jmap.oauth]` section of the account's stored config. Both secrets
/// redact themselves in `Debug`.
#[derive(Debug, Clone, Deserialize)]
pub struct JmapOAuth {
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
    /// The RFC 8707 resource indicator: the canonical URI of the JMAP resource this grant is
    /// for, from the server's RFC 9728 metadata. Persisted because **every refresh must re-send
    /// it**: a server that issues resource-scoped tokens answers `invalid_target` without it, so
    /// dropping it here would break the account at the first refresh rather than at setup.
    /// `None` for a server that publishes no `resource`.
    #[serde(default)]
    pub resource: Option<String>,
}

impl JmapOAuth {
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
            style: AuthStyle::Discovered,
        }
    }

    /// Serializes this grant as the `[jmap.oauth]` sub-table of the account's config TOML.
    /// Manual, like the rest of the account configs, so the secrets need no `Serialize` impl.
    pub(super) fn to_table(&self) -> toml::Table {
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
        table
    }
}

/// Builds the shared, self-refreshing token source for a JMAP OAuth `grant`: the JMAP
/// parallel of `google_token_source`, reusing the same provider-neutral
/// [`GraphTokenSource`]. `sink` (optional) receives a rotated refresh token for
/// re-persistence in the host's OS keystore.
///
/// # Errors
///
/// Returns [`AccountError::Jmap`] if the OAuth HTTP client cannot be built.
pub fn jmap_token_source(
    grant: &JmapOAuth,
    account: AccountId,
    sink: Option<Arc<dyn TokenSink>>,
    origin: crate::CredentialOrigin,
) -> Result<Arc<GraphTokenSource>, AccountError> {
    let oauth = OAuthClient::new(grant.provider_config())
        .map_err(|err| AccountError::Jmap(err.to_string()))?;
    Ok(GraphTokenSource::from_parts(
        oauth,
        account,
        grant.refresh_token.expose().to_owned(),
        sink,
        "jmap",
        origin,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> JmapOAuth {
        JmapOAuth {
            client_id: "03be41ae".to_owned(),
            client_secret: None,
            refresh_token: Secret::new("rt-secret".to_owned()),
            authorize_endpoint: "https://api.example.com/oauth/authorize".to_owned(),
            token_endpoint: "https://api.example.com/oauth/refresh".to_owned(),
            redirect_uri: "eu.allodia.mailcal://jmap-oauth".to_owned(),
            scopes: vec![
                "offline_access".to_owned(),
                "urn:ietf:params:oauth:scope:mail".to_owned(),
            ],
            resource: Some("https://api.example.com/jmap/session".to_owned()),
        }
    }

    #[test]
    fn the_provider_config_uses_the_discovered_endpoints_and_no_vendor_extensions() {
        let config = grant().provider_config();
        assert_eq!(
            config.authorize_endpoint,
            "https://api.example.com/oauth/authorize"
        );
        assert_eq!(
            config.token_endpoint,
            "https://api.example.com/oauth/refresh"
        );
        assert_eq!(config.style, AuthStyle::Discovered);
        assert!(config.client_secret.is_none());
        // The RFC 8707 target must survive into the provider config, or every refresh this grant
        // ever makes is rejected with `invalid_target`.
        assert_eq!(
            config.resource.as_deref(),
            Some("https://api.example.com/jmap/session")
        );

        // A Discovered authorization URL carries the RFC params and nothing else; no
        // `prompt`, no `access_type`, no `response_mode`. Guessing at a vendor extension a
        // discovered server never documented is how a working flow starts 400ing.
        let url = config.authorization_url("state", "challenge", None);
        assert!(url.starts_with("https://api.example.com/oauth/authorize?"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("response_type=code"));
        assert!(!url.contains("prompt="));
        assert!(!url.contains("access_type="));
        assert!(!url.contains("response_mode="));
    }

    #[test]
    fn a_login_hint_is_passed_through_when_the_address_is_known() {
        let config = grant().provider_config();
        let url = config.authorization_url("s", "c", Some("alice@example.com"));
        assert!(url.contains("login_hint=alice%40example.com"));
        // …and a blank one is dropped rather than sent empty.
        assert!(
            !config
                .authorization_url("s", "c", Some("  "))
                .contains("login_hint")
        );
    }

    #[test]
    fn debug_never_leaks_either_secret() {
        let mut with_secret = grant();
        with_secret.client_secret = Some(Secret::new("cs-secret".to_owned()));
        let dump = format!("{with_secret:?}");
        assert!(!dump.contains("rt-secret"));
        assert!(!dump.contains("cs-secret"));
    }
}
