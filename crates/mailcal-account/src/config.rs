//! One account's connection config: the IMAP/SMTP/CalDAV endpoints and credentials a
//! host reads from its OS secure store; plus loading it from TOML.
//!
//! The config carries secrets, so it stays out of logs (see [`Secret`]) and out of version
//! control: a real host uses the OS keychain. The account-setup builder that produces this
//! TOML from a host's setup UI lives in [`crate::build_config_toml`].

use std::{
    fmt,
    path::{Path, PathBuf},
};

use engine_core::ids::{AccountId, IdError};
use provider_imap::{Credentials, ImapConfig};
use serde::Deserialize;

use crate::{OAuthGrant, connect_log::connect_logger};

/// A secret string (password/token) that redacts itself in `Debug`, so a config
/// holding it can still derive `Debug` without leaking the secret into logs.
#[derive(Clone, Deserialize)]
pub struct Secret(String);

impl Secret {
    /// Wraps a raw secret: so a caller outside this crate (the bindings, building a
    /// [`MicrosoftConfig`](crate::MicrosoftConfig) from a freshly minted OAuth token)
    /// can construct one.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The underlying secret. Call only at the point of use (building a connection),
    /// never to log or display it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// How a mail connection is secured: TLS from the first byte (implicit TLS, the standard
/// secure ports 993/465), or a cleartext connection upgraded in place with `STARTTLS`
/// (ports 143/587). Mirrors the engine's `ImapSecurity`/`SmtpSecurity` (`provider-imap`),
/// which is what a config built from this ultimately selects. Defaults to implicit TLS, so
/// an account TOML that names no `security` connects exactly as before this field existed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum ConnectionSecurity {
    /// Implicit TLS: the socket is TLS-wrapped before the greeting (ports 993/465).
    #[default]
    #[serde(rename = "implicit-tls")]
    ImplicitTls,
    /// STARTTLS: connect in the clear, negotiate the TLS upgrade, then authenticate (ports
    /// 143/587). Credentials never cross the wire before the upgrade.
    #[serde(rename = "starttls")]
    StartTls,
}

/// One account's connection config: the IMAP mail endpoint (required), and optional
/// SMTP submission and CalDAV calendar endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountConfig {
    /// The IMAP mail endpoint.
    pub imap: ImapAccount,
    /// The SMTP submission endpoint, if mail-send is configured.
    #[serde(default)]
    pub smtp: Option<SmtpAccount>,
    /// The CalDAV calendar endpoint, if calendar sync is configured.
    #[serde(default)]
    pub caldav: Option<CalDavAccount>,
}

/// An IMAP endpoint: the `host:port` to dial, the TLS server name, and credentials.
///
/// **Exactly one of `password` and `oauth` is set**, and which one is a property of the
/// server rather than a preference: the setup screen reads what the server advertises before
/// it asks for anything ([`docs/mail-oauth.md`](../../../docs/mail-oauth.md)). An account
/// stored before OAuth existed has a `password` and keeps working untouched.
#[derive(Debug, Clone, Deserialize)]
pub struct ImapAccount {
    /// The dial address, `host:port` (e.g. `imap.soverin.net:993`).
    pub addr: String,
    /// The TLS server name for SNI/verification (e.g. `imap.soverin.net`).
    pub server_name: String,
    /// The login username (the full email address). Present on both credential shapes: an
    /// OAuth account still names the mailbox its token was issued for, which is what the SASL
    /// response carries as its `authzid`.
    pub username: String,
    /// The login password (or app-specific password). `None` for an OAuth account, which
    /// stores no long-lived secret of its own.
    #[serde(default)]
    pub password: Option<Secret>,
    /// The browser sign-in grant, when the account authenticates by OAuth. Takes precedence
    /// over `password`: a fresh access token is minted from it for every connection, so
    /// nothing long-lived is presented to the server.
    #[serde(default)]
    pub oauth: Option<OAuthGrant>,
    /// How the IMAP connection is secured; defaults to implicit TLS (port 993).
    #[serde(default)]
    pub security: ConnectionSecurity,
}

/// An SMTP submission endpoint over implicit TLS or STARTTLS.
#[derive(Debug, Clone, Deserialize)]
pub struct SmtpAccount {
    /// The dial address, `host:port` (e.g. `smtp.soverin.net:465`).
    pub addr: String,
    /// The TLS server name for SNI/verification (e.g. `smtp.soverin.net`).
    pub server_name: String,
    /// How the SMTP submission connection is secured; defaults to implicit TLS (port 465).
    #[serde(default)]
    pub security: ConnectionSecurity,
}

/// A CalDAV calendar endpoint, authenticating the same way the account's mail does.
#[derive(Debug, Clone, Deserialize)]
pub struct CalDavAccount {
    /// The base URL (e.g. `https://caldav.soverin.net`).
    pub base_url: String,
    /// The login username (the full email address).
    pub username: String,
    /// The login password (or app-specific password). `None` on an OAuth account, which
    /// presents the mail grant's bearer token here instead: there is no password to reuse,
    /// and the profile's `calendars` scope is requested precisely so this works
    /// ([`docs/mail-oauth.md`](../../../docs/mail-oauth.md)).
    #[serde(default)]
    pub password: Option<Secret>,
    /// The calendar collection to sync (defaults to discovery's primary).
    #[serde(default)]
    pub calendar: Option<String>,
}

impl AccountConfig {
    /// Whether this account authenticates by browser sign-in rather than by a stored password.
    ///
    /// An OAuth account takes a different connect path entirely: a fresh access token is
    /// minted for every dial, and an authentication failure means "refresh and redial" rather
    /// than "the password is wrong".
    #[must_use]
    pub fn is_oauth(&self) -> bool {
        self.imap.oauth.is_some()
    }

    /// Clones this account with `password` applied to every endpoint that shares the account's
    /// login. SMTP takes its credentials from the IMAP half, so only IMAP and CalDAV carry a
    /// secret in the stored config.
    ///
    /// On an OAuth account this is a **no-op**: there is no password to replace, and writing
    /// one would leave an account with both credentials and no way to say which is meant. The
    /// repair path for an OAuth account is a re-authorisation, not a re-typed secret.
    #[must_use]
    pub fn with_password(&self, password: &str) -> Self {
        let mut updated = self.clone();
        if updated.is_oauth() {
            return updated;
        }
        let password = Secret::new(password.to_owned());
        updated.imap.password = Some(password.clone());
        if let Some(caldav) = &mut updated.caldav {
            caldav.password = Some(password);
        }
        updated
    }

    /// Clones this account with `grant` replacing its OAuth grant: the re-authorisation
    /// counterpart of [`with_password`](Self::with_password), and how a rotated refresh token
    /// or a re-consent is written back.
    #[must_use]
    pub fn with_grant(&self, grant: OAuthGrant) -> Self {
        let mut updated = self.clone();
        updated.imap.oauth = Some(grant);
        // A grant supersedes any stored password: leaving one behind would make "which
        // credential does this account use?" a question with two answers.
        updated.imap.password = None;
        if let Some(caldav) = &mut updated.caldav {
            caldav.password = None;
        }
        updated
    }

    /// Serializes the account to the secure-store TOML shape, including its redacted-in-memory
    /// credentials. Manual construction keeps [`Secret`] deliberately non-serializable, so a
    /// caller cannot accidentally serialize one outside this explicit storage boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Serialize`] if TOML encoding fails.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        let mut imap = server_table(&self.imap.addr, &self.imap.server_name, self.imap.security);
        imap.insert("username".into(), self.imap.username.clone().into());
        if let Some(password) = &self.imap.password {
            imap.insert("password".into(), password.expose().to_owned().into());
        }
        // Written last so the sub-table lands after the scalar keys: a TOML table must not be
        // followed by keys belonging to its parent, and `toml` orders a map's entries as it
        // finds them.
        if let Some(grant) = &self.imap.oauth {
            imap.insert("oauth".into(), grant.to_table().into());
        }
        let mut root = toml::Table::new();
        root.insert("imap".into(), imap.into());

        if let Some(smtp) = &self.smtp {
            root.insert(
                "smtp".into(),
                server_table(&smtp.addr, &smtp.server_name, smtp.security).into(),
            );
        }
        if let Some(caldav) = &self.caldav {
            let mut table = toml::Table::new();
            table.insert("base_url".into(), caldav.base_url.clone().into());
            table.insert("username".into(), caldav.username.clone().into());
            if let Some(password) = &caldav.password {
                table.insert("password".into(), password.expose().to_owned().into());
            }
            if let Some(calendar) = &caldav.calendar {
                table.insert("calendar".into(), calendar.clone().into());
            }
            root.insert("caldav".into(), table.into());
        }
        Ok(toml::to_string(&root)?)
    }

    /// The engine credentials for a **password** account, or `None` when this account signs in
    /// with OAuth (whose access token is minted per dial and cannot come from stored config).
    #[must_use]
    pub fn imap_password_credentials(&self) -> Option<Credentials> {
        self.imap
            .password
            .as_ref()
            .map(|password| Credentials::password(&self.imap.username, password.expose()))
    }

    /// Builds the engine [`ImapConfig`] for this account with `credentials`, wiring SMTP
    /// submission when configured.
    ///
    /// The credential is a parameter rather than something read out of `self` because an OAuth
    /// account's is **not stored**: its access token is minted for each dial and expires within
    /// the hour, so a config built once and reused would authenticate exactly as long as its
    /// first token lived. Every dial therefore passes a freshly resolved credential, and a
    /// password account passes [`imap_password_credentials`](Self::imap_password_credentials).
    ///
    /// The connect observer rides on the config, so every connection built from it is traced;
    /// the sync provider, the `IDLE` watcher, and each re-dial after a dropped session.
    #[must_use]
    pub fn imap_config(&self, credentials: Credentials) -> ImapConfig {
        let mut config = ImapConfig::new(
            self.imap.addr.clone(),
            self.imap.server_name.clone(),
            credentials,
        )
        .with_connect_observer(connect_logger("imap"));
        if self.imap.security == ConnectionSecurity::StartTls {
            config = config.with_starttls();
        }
        match &self.smtp {
            Some(smtp) => match smtp.security {
                ConnectionSecurity::ImplicitTls => {
                    config.with_smtp_tls(smtp.addr.clone(), smtp.server_name.clone())
                }
                ConnectionSecurity::StartTls => {
                    config.with_smtp_starttls(smtp.addr.clone(), smtp.server_name.clone())
                }
            },
            None => config,
        }
    }

    /// Derives this account's stable [`AccountId`] from its IMAP login: the **username
    /// and host together**, both **lowercased**. The host disambiguates the same username
    /// on different servers (e.g. `alice@example.com` on two providers), and lowercasing
    /// makes the id stable across case drift in the typed username, since neither IMAP
    /// usernames nor hostnames are case-sensitive in practice. Hostnames never contain
    /// `@`, so `username@host` splits back unambiguously; distinct `(username, host)`
    /// pairs never collide. This id scopes everything in the shared engine store, so it
    /// must be the same for "the same mailbox" across launches.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] only if the derived id is empty (an empty username and host).
    pub fn account_id(&self) -> Result<AccountId, IdError> {
        let username = self.imap.username.trim().to_lowercase();
        let host = self.imap.server_name.trim().to_lowercase();
        AccountId::try_from(format!("{username}@{host}").as_str())
    }
}

fn server_table(addr: &str, server_name: &str, security: ConnectionSecurity) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert("addr".into(), addr.to_owned().into());
    table.insert("server_name".into(), server_name.to_owned().into());
    if security == ConnectionSecurity::StartTls {
        table.insert("security".into(), "starttls".into());
    }
    table
}

/// Loads an [`AccountConfig`] from a TOML file at `path`.
///
/// # Errors
///
/// Returns [`ConfigError`] if the file cannot be read or is not valid config.
pub fn load(path: impl AsRef<Path>) -> Result<AccountConfig, ConfigError> {
    let text = std::fs::read_to_string(path)?;
    load_str(&text)
}

/// Parses an [`AccountConfig`] from a TOML string: the in-memory form a host reads
/// from its OS secure store (Keychain / EncryptedSharedPreferences) rather than a
/// plaintext file on disk.
///
/// # Errors
///
/// Returns [`ConfigError`] if the text is not valid config.
pub fn load_str(text: &str) -> Result<AccountConfig, ConfigError> {
    Ok(toml::from_str(text)?)
}

/// The default config path, `$HOME/.config/mailcal/account.toml`.
#[must_use]
pub fn default_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".config/mailcal/account.toml")
}

/// An error loading account config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file could not be read.
    #[error("reading config: {0}")]
    Io(#[from] std::io::Error),
    /// The config file was not valid TOML / the expected shape.
    #[error("parsing config: {0}")]
    Parse(#[from] toml::de::Error),
    /// A required account-setup field was empty.
    #[error("missing required field: {0}")]
    Incomplete(&'static str),
    /// The setup fields could not be serialized to TOML.
    #[error("serializing config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
