//! Microsoft 365 (Graph) account config: the OAuth counterpart of [`AccountConfig`]'s
//! password-based IMAP/CalDAV.
//!
//! A Microsoft account carries **no password**: the host runs the OAuth
//! Authorization-Code+PKCE flow (`mailcal-oauth`), and this config persists only the
//! **refresh token** plus the app registration needed to mint access tokens (client id,
//! tenant, redirect URI, scopes) and the account's own address. The short-lived access
//! token is never stored; it is refreshed on demand at connect time
//! ([`crate::graph`]). Like [`AccountConfig`], the secret redacts itself in `Debug`, so
//! a config is safe to log.
//!
//! [`AccountConfig`]: crate::AccountConfig

use engine_api::EmailAddress;
use engine_core::ids::{AccountId, IdError};
use mailcal_oauth::OAuthProviderConfig;
use serde::Deserialize;

use crate::{AccountError, ConfigError, Secret, tls::account_tls};

/// The host sentinel appended to a Microsoft account's address to form its stable
/// [`AccountId`], mirroring how an IMAP account's id is `username@server_name`. Graph
/// mailboxes have no dialable host, so this fixed marker disambiguates a Microsoft
/// account from an IMAP account for the same address (which uses the real IMAP host).
const GRAPH_ID_HOST: &str = "graph.microsoft.com";

/// One Microsoft 365 account's connection config: the app registration, the signed-in
/// address, and the long-lived refresh token. Deserialized from the `[microsoft]`
/// section a host stores in its OS secure store.
#[derive(Debug, Clone, Deserialize)]
pub struct MicrosoftConfig {
    /// The account's own email address (from Graph `/me`), used to name it and derive
    /// its [`AccountId`].
    pub email: String,
    /// The registered application (client) id of the host's Azure app. Not a secret.
    pub client_id: String,
    /// The tenant the app authenticates against (`common`, `organizations`,
    /// `consumers`, or a tenant id).
    pub tenant: String,
    /// The redirect URI the OAuth flow returns to (a custom scheme captured by the
    /// platform auth session).
    pub redirect_uri: String,
    /// The delegated scopes granted for this account.
    pub scopes: Vec<String>,
    /// The OAuth refresh token: the only stored secret; access tokens are minted from
    /// it at connect time and never persisted.
    pub refresh_token: Secret,
}

impl MicrosoftConfig {
    /// Derives this account's stable [`AccountId`] from its **lowercased address** plus
    /// the Graph host sentinel; stable across launches, and distinct from an IMAP
    /// account for the same address (see `GRAPH_ID_HOST`).
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] only if the address is empty (an empty id).
    pub fn account_id(&self) -> Result<AccountId, IdError> {
        let email = self.email.trim().to_lowercase();
        AccountId::try_from(format!("{email}@{GRAPH_ID_HOST}").as_str())
    }

    /// This account's identity for the app's `Account` (its own address).
    #[must_use]
    pub fn identity(&self) -> EmailAddress {
        EmailAddress::new(self.email.clone())
    }

    /// Builds the `mailcal-oauth` provider config for this account (the endpoints +
    /// client + scopes), so the token source can refresh its access token.
    #[must_use]
    pub fn provider_config(&self) -> OAuthProviderConfig {
        let scopes: Vec<&str> = self.scopes.iter().map(String::as_str).collect();
        OAuthProviderConfig::microsoft(
            self.client_id.clone(),
            &self.tenant,
            self.redirect_uri.clone(),
            &scopes,
        )
    }

    /// Serializes this config to the `[microsoft]` TOML a host stores in its OS secure
    /// store: the inverse of deserialization. Manual (like [`crate::build_config_toml`])
    /// so the secret needs no `Serialize` impl.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Serialize`] on a serialization error.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let mut microsoft = toml::Table::new();
        microsoft.insert("email".into(), self.email.clone().into());
        microsoft.insert("client_id".into(), self.client_id.clone().into());
        microsoft.insert("tenant".into(), self.tenant.clone().into());
        microsoft.insert("redirect_uri".into(), self.redirect_uri.clone().into());
        microsoft.insert(
            "scopes".into(),
            toml::Value::Array(self.scopes.iter().map(|s| s.clone().into()).collect()),
        );
        microsoft.insert(
            "refresh_token".into(),
            self.refresh_token.expose().to_owned().into(),
        );
        let mut root = toml::Table::new();
        root.insert("microsoft".into(), microsoft.into());
        Ok(toml::to_string(&root)?)
    }
}

/// A parsed Microsoft account config document (`[microsoft]` at its root), the form a
/// host reads back from its secure store.
#[derive(Debug, Clone, Deserialize)]
struct MicrosoftDocument {
    microsoft: MicrosoftConfig,
}

/// Parses a [`MicrosoftConfig`] from its stored TOML string.
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] if the text is not a valid `[microsoft]` config.
pub fn load_microsoft_str(text: &str) -> Result<MicrosoftConfig, ConfigError> {
    let doc: MicrosoftDocument = toml::from_str(text)?;
    Ok(doc.microsoft)
}

/// Looks up the signed-in account's own email address via Graph `GET /me`, so a freshly
/// authorised account can be named and keyed without asking the user to type it.
///
/// Prefers the `mail` property; falls back to `userPrincipalName` (some accounts have no
/// `mail` set). Authenticates with the bearer `access_token` just obtained in the flow.
///
/// # Errors
///
/// Returns [`AccountError::Graph`] if the request fails, is non-2xx, or returns no
/// usable address.
pub async fn fetch_primary_address(access_token: &str) -> Result<String, AccountError> {
    let http = account_tls()?
        .reqwest_builder()
        .build()
        .map_err(|err| AccountError::Graph(format!("me lookup client: {err}")))?;
    let resp = http
        .get("https://graph.microsoft.com/v1.0/me?$select=mail,userPrincipalName")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| AccountError::Graph(format!("me lookup: {err}")))?;
    if !resp.status().is_success() {
        return Err(AccountError::Graph(format!(
            "me lookup: http {}",
            resp.status().as_u16()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|err| AccountError::Graph(format!("me lookup decode: {err}")))?;
    let address = body
        .get("mail")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            body.get("userPrincipalName")
                .and_then(serde_json::Value::as_str)
        })
        .ok_or_else(|| AccountError::Graph("me lookup returned no address".to_owned()))?;
    Ok(address.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MicrosoftConfig {
        MicrosoftConfig {
            email: "Alice@Example.com".to_owned(),
            client_id: "client-abc".to_owned(),
            tenant: "common".to_owned(),
            redirect_uri: "eu.allodia.mailcal://oauth".to_owned(),
            scopes: vec![
                "offline_access".to_owned(),
                "https://graph.microsoft.com/Mail.Read".to_owned(),
            ],
            refresh_token: Secret::new("secret-refresh-token".to_owned()),
        }
    }

    #[test]
    fn account_id_lowercases_and_appends_the_graph_host() {
        assert_eq!(
            config().account_id().unwrap().as_str(),
            "alice@example.com@graph.microsoft.com"
        );
    }

    #[test]
    fn a_microsoft_account_id_differs_from_an_imap_account_for_the_same_address() {
        // The Graph sentinel host keeps a Microsoft account distinct from an IMAP one
        // for the same address (which keys on the real imap server).
        let ms = config().account_id().unwrap();
        let imap = AccountId::try_from("alice@example.com@imap.example.com").unwrap();
        assert_ne!(ms, imap);
    }

    #[test]
    fn to_toml_round_trips_through_load_including_the_secret() {
        let toml = config().to_toml().unwrap();
        let parsed = load_microsoft_str(&toml).unwrap();
        assert_eq!(parsed.email, "Alice@Example.com");
        assert_eq!(parsed.client_id, "client-abc");
        assert_eq!(parsed.tenant, "common");
        assert_eq!(parsed.redirect_uri, "eu.allodia.mailcal://oauth");
        assert_eq!(parsed.scopes.len(), 2);
        assert_eq!(parsed.refresh_token.expose(), "secret-refresh-token");
    }

    #[test]
    fn debug_never_leaks_the_refresh_token() {
        assert!(!format!("{:?}", config()).contains("secret-refresh-token"));
        assert_eq!(
            format!("{:?}", config().refresh_token),
            "Secret(<redacted>)"
        );
    }

    #[test]
    fn provider_config_builds_microsoft_endpoints_for_the_tenant() {
        let cfg = provider_config_for("contoso.onmicrosoft.com");
        assert!(
            cfg.authorize_endpoint
                .contains("/contoso.onmicrosoft.com/oauth2/v2.0/authorize")
        );
        assert_eq!(cfg.client_id, "client-abc");
        assert_eq!(cfg.redirect_uri, "eu.allodia.mailcal://oauth");
    }

    fn provider_config_for(tenant: &str) -> OAuthProviderConfig {
        let mut c = config();
        c.tenant = tenant.to_owned();
        c.provider_config()
    }
}
