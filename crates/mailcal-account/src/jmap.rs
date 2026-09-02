//! JMAP account config (RFC 8620/8621): the third provider kind, alongside
//! password-based IMAP ([`AccountConfig`]) and OAuth Microsoft Graph
//! ([`MicrosoftConfig`]).
//!
//! A JMAP account is a **base URL + credentials**: the engine's [`JmapProvider`]
//! discovers the API endpoint, the account ids, and the mailbox set from the
//! server's session resource (`/.well-known/jmap`), so the config carries no
//! per-folder or per-endpoint detail. Credentials are either **HTTP Basic** (the
//! account address + a password / app-specific password; what Stalwart and most
//! JMAP servers accept) or an **OAuth bearer token** (an API token, e.g.
//! Fastmail). The single at-rest secret redacts itself in `Debug`, so a config is
//! safe to log.
//!
//! Unlike IMAP/Graph, one [`JmapProvider`] serves the **whole account**; its
//! email scope is account-wide (`JmapType { account, Email }`), and each message
//! carries its `mailboxIds` membership: so a single provider syncs every folder,
//! and there are no per-role folder providers to bind.
//!
//! [`AccountConfig`]: crate::AccountConfig
//! [`MicrosoftConfig`]: crate::MicrosoftConfig

use engine_api::EmailAddress;
use engine_core::ids::{AccountId, IdError};
use provider_jmap::{Credentials, JmapConfig};
use serde::Deserialize;

mod connect;
mod refreshing;
mod refreshing_contacts;
mod refreshing_provider;
mod setup;

pub use connect::{
    connect_jmap_calendar_providers, connect_jmap_contact_providers, connect_jmap_folder,
    connect_jmap_mail_providers,
};
pub use setup::{JmapSetup, build_jmap_config_toml, jmap_base_url};

use crate::{
    ConfigError, OAuthGrant, Secret, connect_log::connect_logger, throttle::account_retry,
};

/// The id-scheme tag woven into a JMAP account's [`AccountId`], so a JMAP account
/// never collides with an IMAP or Microsoft account for the same address on the
/// same host. IMAP keys on `address@host`, Microsoft on `address@graph.microsoft.com`;
/// JMAP on `address@jmap:host`. The tag can never appear in a real IMAP `server_name`,
/// so the three id spaces stay disjoint.
const JMAP_ID_SCHEME: &str = "jmap";

/// One JMAP account's connection config: the account address, the server base URL,
/// and exactly one secret: a Basic-auth `password` **or** a bearer `token`.
/// Deserialized from the `[jmap]` section a host stores in its OS secure store.
///
/// New setups always store the secret as `password` (with `email` as the username),
/// because the engine negotiates the wire scheme from the server's `WWW-Authenticate`
/// challenge and a username-bearing credential can be presented **either** way, while a
/// bare token can only ever be Bearer. `token` remains only to read back configs stored
/// before that collapse; see [`build_jmap_config_toml`].
#[derive(Debug, Clone, Deserialize)]
pub struct JmapAccountConfig {
    /// The account's email address: the login username for Basic auth, the
    /// account's identity, and the basis of its [`AccountId`].
    pub email: String,
    /// The JMAP server base URL to connect to (e.g. `https://mail.example.com`, or
    /// `http://127.0.0.1:28080` for the local test server). The session resource,
    /// API URL, and account ids are discovered from it.
    pub base_url: String,
    /// The account's secret: a login password, an app-specific password, or an API
    /// token. Paired with `email` as the Basic username, so the engine can present it
    /// under **either** scheme the server challenges for. What every new setup writes.
    #[serde(default)]
    pub password: Option<Secret>,
    /// A bearer-only API token, from a config stored **before** the two secret fields
    /// were collapsed. Read for backward compatibility and still honoured (it takes
    /// precedence over `password`), but never written by a new setup: lacking a
    /// username it can only ever be presented as Bearer.
    #[serde(default)]
    pub token: Option<Secret>,
    /// An OAuth grant, when the account was connected by signing in with the provider
    /// rather than by pasting a secret. Mutually exclusive with `password`/`token` in
    /// practice, and takes precedence: a fresh access token is minted from it for every
    /// connection, so nothing long-lived is presented to the server.
    #[serde(default)]
    pub oauth: Option<OAuthGrant>,
}

impl JmapAccountConfig {
    /// Derives this account's stable [`AccountId`] from its **lowercased address**,
    /// the JMAP scheme tag, and the server host; stable across launches, and
    /// distinct from an IMAP or Microsoft account for the same address (see
    /// `JMAP_ID_SCHEME`). Two JMAP accounts for the same address on different
    /// servers stay distinct (the host is part of the id).
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] only if the derived id is empty.
    pub fn account_id(&self) -> Result<AccountId, IdError> {
        let email = self.email.trim().to_lowercase();
        let host = base_host(&self.base_url);
        AccountId::try_from(format!("{email}@{JMAP_ID_SCHEME}:{host}").as_str())
    }

    /// This account's identity for the app's `Account` (its own address).
    #[must_use]
    pub fn identity(&self) -> EmailAddress {
        EmailAddress::new(self.email.clone())
    }

    /// Whether this account authenticates by OAuth (a discovered browser sign-in) rather
    /// than by a stored secret. An OAuth account mints a short-lived access token per
    /// connection, so it takes a different connect path entirely.
    #[must_use]
    pub fn is_oauth(&self) -> bool {
        self.oauth.is_some()
    }

    /// Builds the engine credentials **for the stored-secret path**: a bearer token when a
    /// pre-collapse one is stored, else HTTP Basic with the account address as the username.
    /// Never consulted for an OAuth account, whose token comes from the token source.
    fn credentials(&self) -> Credentials {
        match &self.token {
            Some(token) => Credentials::bearer(token.expose().to_owned()),
            None => Credentials::basic(
                self.email.clone(),
                self.password.as_ref().map_or("", Secret::expose).to_owned(),
            ),
        }
    }

    /// Builds the engine [`JmapConfig`] for this account (base URL + credentials);
    /// everything else is discovered from the session.
    fn engine_config(&self, tls: engine_tls::TlsClientConfig) -> JmapConfig {
        JmapConfig::new(self.base_url.clone(), self.credentials())
            .with_tls(tls)
            .with_retry(account_retry())
            .with_connect_observer(connect_logger("jmap"))
    }

    /// Serializes this config to the `[jmap]` TOML a host stores in its OS secure
    /// store: the inverse of deserialization. Manual (like [`MicrosoftConfig::to_toml`])
    /// so the secret needs no `Serialize` impl. Emits only the secret that is set.
    ///
    /// [`MicrosoftConfig::to_toml`]: crate::MicrosoftConfig::to_toml
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Serialize`] on a serialization error.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let mut jmap = toml::Table::new();
        jmap.insert("email".into(), self.email.clone().into());
        jmap.insert("base_url".into(), self.base_url.clone().into());
        if let Some(token) = &self.token {
            jmap.insert("token".into(), token.expose().to_owned().into());
        } else if let Some(password) = &self.password {
            jmap.insert("password".into(), password.expose().to_owned().into());
        }
        // The grant goes in a `[jmap.oauth]` sub-table. Inserted last so the scalar keys
        // above are not swallowed into it by TOML's table syntax.
        if let Some(oauth) = &self.oauth {
            jmap.insert("oauth".into(), oauth.to_table().into());
        }
        let mut root = toml::Table::new();
        root.insert("jmap".into(), jmap.into());
        Ok(toml::to_string(&root)?)
    }
}

/// The host portion (`host` or `host:port`, lowercased) of a JMAP base URL, for the
/// [`AccountId`]. Deterministic string surgery (no URL parsing) so the derived id
/// is stable: strip an optional scheme, then take everything up to the first `/`.
fn base_host(base_url: &str) -> String {
    let no_scheme = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    no_scheme
        .split('/')
        .next()
        .unwrap_or(no_scheme)
        .trim()
        .to_lowercase()
}

/// A parsed JMAP account config document (`[jmap]` at its root), the form a host
/// reads back from its secure store.
#[derive(Debug, Clone, Deserialize)]
struct JmapDocument {
    jmap: JmapAccountConfig,
}

/// Parses a [`JmapAccountConfig`] from its stored TOML string.
///
/// # Errors
///
/// Returns [`ConfigError::Parse`] if the text is not a valid `[jmap]` config.
pub fn load_jmap_str(text: &str) -> Result<JmapAccountConfig, ConfigError> {
    let doc: JmapDocument = toml::from_str(text)?;
    Ok(doc.jmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_config() -> JmapAccountConfig {
        JmapAccountConfig {
            email: "Alice@Example.com".to_owned(),
            base_url: "https://mail.example.com".to_owned(),
            password: Some(Secret::new("hunter2".to_owned())),
            token: None,
            oauth: None,
        }
    }

    fn token_config() -> JmapAccountConfig {
        JmapAccountConfig {
            email: "alice@example.com".to_owned(),
            base_url: "https://api.fastmail.com".to_owned(),
            password: None,
            token: Some(Secret::new("fmapi-secret".to_owned())),
            oauth: None,
        }
    }

    #[test]
    fn account_id_lowercases_and_tags_the_scheme_and_host() {
        assert_eq!(
            basic_config().account_id().unwrap().as_str(),
            "alice@example.com@jmap:mail.example.com"
        );
    }

    #[test]
    fn account_id_is_distinct_from_imap_and_graph_for_the_same_address() {
        let jmap = basic_config().account_id().unwrap();
        let imap = AccountId::try_from("alice@example.com@mail.example.com").unwrap();
        let graph = AccountId::try_from("alice@example.com@graph.microsoft.com").unwrap();
        assert_ne!(jmap, imap);
        assert_ne!(jmap, graph);
    }

    #[test]
    fn account_id_differs_by_server_for_the_same_address() {
        let a = basic_config().account_id().unwrap();
        let mut other = basic_config();
        other.base_url = "https://jmap.other.net".to_owned();
        assert_ne!(a, other.account_id().unwrap());
    }

    #[test]
    fn base_host_strips_scheme_and_path_and_keeps_the_port() {
        assert_eq!(base_host("http://127.0.0.1:18080"), "127.0.0.1:18080");
        assert_eq!(base_host("https://mail.example.com/"), "mail.example.com");
        assert_eq!(
            base_host("https://Mail.Example.com/jmap"),
            "mail.example.com"
        );
        // A scheme-less host (tolerated) still yields the host.
        assert_eq!(base_host("mail.example.com"), "mail.example.com");
    }

    #[test]
    fn to_toml_round_trips_a_basic_password_account() {
        let toml = basic_config().to_toml().unwrap();
        let parsed = load_jmap_str(&toml).unwrap();
        assert_eq!(parsed.email, "Alice@Example.com");
        assert_eq!(parsed.base_url, "https://mail.example.com");
        assert_eq!(parsed.password.as_ref().unwrap().expose(), "hunter2");
        assert!(parsed.token.is_none());
    }

    #[test]
    fn to_toml_round_trips_a_bearer_token_account() {
        let toml = token_config().to_toml().unwrap();
        let parsed = load_jmap_str(&toml).unwrap();
        assert_eq!(parsed.token.as_ref().unwrap().expose(), "fmapi-secret");
        assert!(parsed.password.is_none());
    }

    #[test]
    fn a_token_takes_precedence_over_a_password_in_credentials_and_toml() {
        // A config with both (unusual) prefers the token, and to_toml emits only it.
        let both = JmapAccountConfig {
            token: Some(Secret::new("tok".to_owned())),
            ..basic_config()
        };
        assert!(matches!(both.credentials(), Credentials::Bearer(_)));
        let parsed = load_jmap_str(&both.to_toml().unwrap()).unwrap();
        assert!(parsed.token.is_some() && parsed.password.is_none());
    }

    #[test]
    fn basic_credentials_use_the_email_as_username() {
        match basic_config().credentials() {
            Credentials::Basic { username, password } => {
                assert_eq!(username, "Alice@Example.com");
                assert_eq!(password, "hunter2");
            }
            Credentials::Bearer(_) => panic!("expected basic credentials"),
        }
    }

    #[test]
    fn debug_never_leaks_the_secret() {
        assert!(!format!("{:?}", basic_config()).contains("hunter2"));
        assert!(!format!("{:?}", token_config()).contains("fmapi-secret"));
    }
}
