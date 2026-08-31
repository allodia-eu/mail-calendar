//! Google (Gmail + Google Calendar) account config: the OAuth counterpart of
//! [`AccountConfig`]'s password-based IMAP/CalDAV, and the sibling of [`MicrosoftConfig`].
//!
//! A Google account carries **no password**: the host runs the OAuth
//! Authorization-Code+PKCE flow (`mailcal-oauth`), and this config persists only the
//! **refresh token** plus the app registration needed to mint access tokens (client id,
//! redirect URI, scopes) and the account's own address. The
//! short-lived access token is never stored; it is refreshed on demand at connect time
//! ([`crate::google`]). Like [`MicrosoftConfig`], the secret redacts itself in `Debug`, so a
//! config is safe to log.
//!
//! [`AccountConfig`]: crate::AccountConfig
//! [`MicrosoftConfig`]: crate::MicrosoftConfig

use engine_api::EmailAddress;
use engine_core::ids::{AccountId, IdError};
use mailcal_oauth::OAuthProviderConfig;
use serde::Deserialize;

use crate::{AccountError, ConfigError, Secret, tls::account_tls};

/// The host sentinel appended to a Google account's address to form its stable [`AccountId`],
/// mirroring [`MicrosoftConfig`](crate::MicrosoftConfig)'s `graph.microsoft.com`. Gmail
/// mailboxes have no dialable host, so this fixed marker disambiguates a Google account from
/// an IMAP (or Microsoft) account for the same address.
const GOOGLE_ID_HOST: &str = "mail.google.com";

/// The Gmail profile endpoint that returns the signed-in account's own `emailAddress`; the
/// Google parallel of Graph's `/me`. Covered by the `https://mail.google.com/` scope, so no
/// separate `openid`/`email` scope is needed to name the account.
const GMAIL_PROFILE_ENDPOINT: &str = "https://gmail.googleapis.com/gmail/v1/users/me/profile";

/// One Google account's connection config: the app registration, the signed-in address, and
/// the long-lived refresh token. Deserialized from the `[google]` section a host stores in its
/// OS secure store.
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleConfig {
    /// The account's own email address (from the Gmail profile), used to name it and derive
    /// its [`AccountId`].
    pub email: String,
    /// The registered OAuth client id of the host's Google Cloud app. Not a secret.
    pub client_id: String,
    /// The **non-confidential** Google _Desktop_ client secret, when this account was connected
    /// through the macOS/Windows loopback flow. `None` for an iOS/Android client (which needs
    /// none). Persisted so a token refresh can re-send it; Google's token endpoint requires it
    /// on the refresh grant too, or the account would break ~1h after setup. This is *not* a
    /// stored credential in the sense the refresh token is: Google documents a Desktop client's
    /// secret as not confidential (embedded in the app's source). See
    /// <https://developers.google.com/identity/protocols/oauth2#installed>.
    #[serde(default)]
    pub client_secret: Option<String>,
    /// The redirect URI the OAuth flow returns to (a custom scheme, or a loopback URL for the
    /// Desktop client).
    pub redirect_uri: String,
    /// The delegated scopes granted for this account.
    pub scopes: Vec<String>,
    /// The OAuth refresh token: the only stored secret; access tokens are minted from it at
    /// connect time and never persisted.
    pub refresh_token: Secret,
}

impl GoogleConfig {
    /// Derives this account's stable [`AccountId`] from its **lowercased address** plus the
    /// Google host sentinel; stable across launches, and distinct from an IMAP (or Microsoft)
    /// account for the same address (see `GOOGLE_ID_HOST`).
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] only if the address is empty (an empty id).
    pub fn account_id(&self) -> Result<AccountId, IdError> {
        let email = self.email.trim().to_lowercase();
        AccountId::try_from(format!("{email}@{GOOGLE_ID_HOST}").as_str())
    }

    /// This account's identity for the app's `Account` (its own address).
    #[must_use]
    pub fn identity(&self) -> EmailAddress {
        EmailAddress::new(self.email.clone())
    }

    /// Builds the `mailcal-oauth` provider config for this account (the Google endpoints +
    /// client + scopes), so the token source can refresh its access token.
    #[must_use]
    pub fn provider_config(&self) -> OAuthProviderConfig {
        let scopes: Vec<&str> = self.scopes.iter().map(String::as_str).collect();
        OAuthProviderConfig::google(
            self.client_id.clone(),
            self.client_secret.clone(),
            self.redirect_uri.clone(),
            &scopes,
        )
    }

    /// Serializes this config to the `[google]` TOML a host stores in its OS secure store; the
    /// inverse of deserialization. Manual (like [`MicrosoftConfig::to_toml`]) so the secret
    /// needs no `Serialize` impl.
    ///
    /// [`MicrosoftConfig::to_toml`]: crate::MicrosoftConfig::to_toml
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Serialize`] on a serialization error.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let mut google = toml::Table::new();
        google.insert("email".into(), self.email.clone().into());
        google.insert("client_id".into(), self.client_id.clone().into());
        // Non-confidential Desktop secret; persisted only when the account carries one (macOS/
        // Windows loopback), so an iOS/Android account's TOML stays free of the key.
        if let Some(secret) = &self.client_secret {
            google.insert("client_secret".into(), secret.clone().into());
        }
        google.insert("redirect_uri".into(), self.redirect_uri.clone().into());
        google.insert(
            "scopes".into(),
            toml::Value::Array(self.scopes.iter().map(|s| s.clone().into()).collect()),
        );
        google.insert(
            "refresh_token".into(),
            self.refresh_token.expose().to_owned().into(),
        );
        let mut root = toml::Table::new();
        root.insert("google".into(), google.into());
        Ok(toml::to_string(&root)?)
    }
}

/// A parsed Google account config document (`[google]` at its root), the form a host reads
/// back from its secure store.
#[derive(Debug, Clone, Deserialize)]
struct GoogleDocument {
    google: GoogleConfig,
}

/// Parses a [`GoogleConfig`] from its stored TOML string.
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] if the text is not a valid `[google]` config.
pub fn load_google_str(text: &str) -> Result<GoogleConfig, ConfigError> {
    let doc: GoogleDocument = toml::from_str(text)?;
    Ok(doc.google)
}

/// Looks up the signed-in account's own email address via the Gmail `users/me/profile`
/// endpoint, so a freshly authorised account can be named and keyed without asking the user to
/// type it: the Google parallel of [`fetch_primary_address`](crate::fetch_primary_address).
/// Authenticates with the bearer `access_token` just obtained in the flow.
///
/// # Errors
///
/// Returns [`AccountError::Google`] if the request fails, is non-2xx, or returns no address.
pub async fn fetch_google_primary_address(access_token: &str) -> Result<String, AccountError> {
    let http = account_tls()?
        .reqwest_builder()
        .build()
        .map_err(|err| AccountError::Google(format!("profile lookup client: {err}")))?;
    let resp = http
        .get(GMAIL_PROFILE_ENDPOINT)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| AccountError::Google(format!("profile lookup: {err}")))?;
    if !resp.status().is_success() {
        return Err(AccountError::Google(format!(
            "profile lookup: http {}",
            resp.status().as_u16()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|err| AccountError::Google(format!("profile lookup decode: {err}")))?;
    let address = body
        .get("emailAddress")
        .and_then(serde_json::Value::as_str)
        .filter(|address| !address.is_empty())
        .ok_or_else(|| AccountError::Google("profile lookup returned no address".to_owned()))?;
    Ok(address.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> GoogleConfig {
        GoogleConfig {
            email: "Alice@Gmail.com".to_owned(),
            client_id: "google-client".to_owned(),
            client_secret: None,
            redirect_uri: "com.googleusercontent.apps.google-client:/oauth2redirect".to_owned(),
            scopes: vec![
                "https://mail.google.com/".to_owned(),
                "https://www.googleapis.com/auth/calendar".to_owned(),
            ],
            refresh_token: Secret::new("secret-refresh-token".to_owned()),
        }
    }

    #[test]
    fn account_id_lowercases_and_appends_the_google_host() {
        assert_eq!(
            config().account_id().unwrap().as_str(),
            "alice@gmail.com@mail.google.com"
        );
    }

    #[test]
    fn a_google_account_id_differs_from_an_imap_account_for_the_same_address() {
        let google = config().account_id().unwrap();
        let imap = AccountId::try_from("alice@gmail.com@imap.gmail.com").unwrap();
        assert_ne!(google, imap);
    }

    #[test]
    fn to_toml_round_trips_through_load() {
        let toml = config().to_toml().unwrap();
        let parsed = load_google_str(&toml).unwrap();
        assert_eq!(parsed.email, "Alice@Gmail.com");
        assert_eq!(parsed.client_id, "google-client");
        assert_eq!(parsed.scopes.len(), 2);
        assert_eq!(parsed.refresh_token.expose(), "secret-refresh-token");
    }

    #[test]
    fn debug_never_leaks_the_refresh_token() {
        assert!(!format!("{:?}", config()).contains("secret-refresh-token"));
    }

    #[test]
    fn a_desktop_client_secret_round_trips_and_reaches_the_provider_config() {
        // A macOS/Windows Desktop account carries the non-confidential secret; it must survive
        // to_toml → load and flow into the provider config so the refresh grant can re-send it.
        let mut cfg = config();
        cfg.client_secret = Some("GOCSPX-desktop".to_owned());
        let parsed = load_google_str(&cfg.to_toml().unwrap()).unwrap();
        assert_eq!(parsed.client_secret.as_deref(), Some("GOCSPX-desktop"));
        assert_eq!(
            parsed.provider_config().client_secret.as_deref(),
            Some("GOCSPX-desktop")
        );
    }

    #[test]
    fn a_secretless_account_omits_the_client_secret_key_and_still_loads() {
        // An iOS/Android account (client_secret: None) must not write the key, and an older
        // stored config that predates the field must still parse (serde default → None).
        let toml = config().to_toml().unwrap();
        assert!(!toml.contains("client_secret"));
        assert!(load_google_str(&toml).unwrap().client_secret.is_none());
    }

    #[test]
    fn provider_config_builds_the_fixed_google_endpoints() {
        let cfg = config().provider_config();
        assert_eq!(
            cfg.authorize_endpoint,
            "https://accounts.google.com/o/oauth2/v2/auth"
        );
        assert_eq!(cfg.client_id, "google-client");
        // A secretless (iOS/Android) account carries no client secret through to the exchange.
        assert!(cfg.client_secret.is_none());
    }
}
