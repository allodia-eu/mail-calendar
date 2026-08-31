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
use provider_imap::ImapConfig;
use serde::Deserialize;

use crate::connect_log::connect_logger;

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
#[derive(Debug, Clone, Deserialize)]
pub struct ImapAccount {
    /// The dial address, `host:port` (e.g. `imap.soverin.net:993`).
    pub addr: String,
    /// The TLS server name for SNI/verification (e.g. `imap.soverin.net`).
    pub server_name: String,
    /// The login username (the full email address).
    pub username: String,
    /// The login password (or app-specific password).
    pub password: Secret,
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

/// A CalDAV calendar endpoint with Basic-auth credentials.
#[derive(Debug, Clone, Deserialize)]
pub struct CalDavAccount {
    /// The base URL (e.g. `https://caldav.soverin.net`).
    pub base_url: String,
    /// The login username (the full email address).
    pub username: String,
    /// The login password (or app-specific password).
    pub password: Secret,
    /// The calendar collection to sync (defaults to discovery's primary).
    #[serde(default)]
    pub calendar: Option<String>,
}

impl AccountConfig {
    /// Clones this account with `password` applied to every endpoint that shares the account's
    /// login. SMTP takes its credentials from the IMAP half, so only IMAP and CalDAV carry a
    /// secret in the stored config.
    #[must_use]
    pub fn with_password(&self, password: &str) -> Self {
        let mut updated = self.clone();
        let password = Secret::new(password.to_owned());
        updated.imap.password = password.clone();
        if let Some(caldav) = &mut updated.caldav {
            caldav.password = password;
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
        imap.insert(
            "password".into(),
            self.imap.password.expose().to_owned().into(),
        );
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
            table.insert(
                "password".into(),
                caldav.password.expose().to_owned().into(),
            );
            if let Some(calendar) = &caldav.calendar {
                table.insert("calendar".into(), calendar.clone().into());
            }
            root.insert("caldav".into(), table.into());
        }
        Ok(toml::to_string(&root)?)
    }

    /// Builds the engine [`ImapConfig`] for this account, wiring SMTP submission when
    /// configured.
    ///
    /// The connect observer rides on the config, so every connection built from it is traced;
    /// the sync provider, the `IDLE` watcher, and each re-dial after a dropped session.
    #[must_use]
    pub fn imap_config(&self) -> ImapConfig {
        let mut config = ImapConfig::new(
            self.imap.addr.clone(),
            self.imap.server_name.clone(),
            self.imap.username.clone(),
            self.imap.password.expose().to_owned(),
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
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[imap]
addr = "imap.soverin.net:993"
server_name = "imap.soverin.net"
username = "you@example.com"
password = "hunter2"

[smtp]
addr = "smtp.soverin.net:465"
server_name = "smtp.soverin.net"

[caldav]
base_url = "https://caldav.soverin.net"
username = "you@example.com"
password = "hunter2"
"#;

    #[test]
    fn parses_a_full_account_and_redacts_secrets() {
        let config: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
        assert_eq!(config.imap.addr, "imap.soverin.net:993");
        assert_eq!(config.imap.username, "you@example.com");
        assert_eq!(config.imap.password.expose(), "hunter2");

        let smtp = config.smtp.as_ref().expect("smtp present");
        assert_eq!(smtp.addr, "smtp.soverin.net:465");

        let caldav = config.caldav.as_ref().expect("caldav present");
        assert_eq!(caldav.base_url, "https://caldav.soverin.net");
        assert!(caldav.calendar.is_none());

        // Secrets never appear in Debug output (so logging a config is safe).
        let dump = format!("{config:?}");
        assert!(!dump.contains("hunter2"));
        assert_eq!(format!("{:?}", config.imap.password), "Secret(<redacted>)");

        // Builds the engine config without SMTP-absent branching surprises.
        let _ = config.imap_config();
    }

    #[test]
    fn security_defaults_to_implicit_tls_and_parses_starttls() {
        // An account TOML with no `security` key connects exactly as before this field
        // existed: implicit TLS on both transports.
        let default_tls: AccountConfig = toml::from_str(SAMPLE).expect("valid config");
        assert_eq!(default_tls.imap.security, ConnectionSecurity::ImplicitTls);
        assert_eq!(
            default_tls.smtp.as_ref().unwrap().security,
            ConnectionSecurity::ImplicitTls
        );

        // An explicit `security = "starttls"` on the IMAP-143 / submission-587 ports parses
        // and drives the engine's STARTTLS builders (exercised via `imap_config`).
        let starttls: AccountConfig = toml::from_str(
            "[imap]\naddr=\"mail.example.com:143\"\nserver_name=\"mail.example.com\"\n\
             username=\"u\"\npassword=\"p\"\nsecurity=\"starttls\"\n\
             [smtp]\naddr=\"mail.example.com:587\"\nserver_name=\"mail.example.com\"\n\
             security=\"starttls\"\n",
        )
        .expect("valid config");
        assert_eq!(starttls.imap.security, ConnectionSecurity::StartTls);
        assert_eq!(
            starttls.smtp.as_ref().unwrap().security,
            ConnectionSecurity::StartTls
        );
        let _ = starttls.imap_config();
    }

    #[test]
    fn parses_an_imap_only_account() {
        let config: AccountConfig = toml::from_str(
            "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"p\"\n",
        )
        .expect("valid config");
        assert!(config.smtp.is_none() && config.caldav.is_none());
        let _ = config.imap_config();
    }

    #[test]
    fn parses_an_explicit_caldav_calendar() {
        let config: AccountConfig = toml::from_str(
            "[imap]\naddr=\"h:993\"\nserver_name=\"h\"\nusername=\"u\"\npassword=\"p\"\n\
             [caldav]\nbase_url=\"https://dav.example.com\"\nusername=\"u\"\npassword=\"p\"\n\
             calendar=\"work\"\n",
        )
        .expect("valid config");
        let caldav = config.caldav.as_ref().expect("caldav present");
        assert_eq!(caldav.calendar.as_deref(), Some("work"));
    }

    #[test]
    fn replacing_a_password_preserves_every_endpoint_and_updates_caldav_too() {
        let original: AccountConfig = toml::from_str(
            "[imap]\naddr=\"mail.example.com:143\"\nserver_name=\"imap.example.com\"\n\
             username=\"alice@example.com\"\npassword=\"old\"\nsecurity=\"starttls\"\n\
             [smtp]\naddr=\"submit.example.com:587\"\nserver_name=\"smtp.example.com\"\n\
             security=\"starttls\"\n\
             [caldav]\nbase_url=\"https://dav.example.com/root\"\n\
             username=\"calendar-alias\"\npassword=\"old\"\ncalendar=\"work\"\n",
        )
        .expect("valid config");

        let updated = original
            .with_password("new\"secret\\value")
            .to_toml()
            .expect("serializable config");
        let parsed = load_str(&updated).expect("replacement config round-trips");

        assert_eq!(parsed.imap.password.expose(), "new\"secret\\value");
        assert_eq!(
            parsed.caldav.as_ref().unwrap().password.expose(),
            "new\"secret\\value"
        );
        assert_eq!(parsed.imap.addr, "mail.example.com:143");
        assert_eq!(parsed.imap.server_name, "imap.example.com");
        assert_eq!(parsed.imap.security, ConnectionSecurity::StartTls);
        assert_eq!(parsed.smtp.as_ref().unwrap().addr, "submit.example.com:587");
        assert_eq!(parsed.caldav.as_ref().unwrap().username, "calendar-alias");
        assert_eq!(
            parsed.caldav.as_ref().unwrap().calendar.as_deref(),
            Some("work")
        );
    }

    fn config_with(username: &str, server_name: &str) -> AccountConfig {
        toml::from_str(&format!(
            "[imap]\naddr=\"{server_name}:993\"\nserver_name=\"{server_name}\"\n\
             username=\"{username}\"\npassword=\"p\"\n",
        ))
        .expect("valid config")
    }

    #[test]
    fn account_id_is_case_insensitive_in_username_and_host() {
        // Case drift in the typed username (or host) must not mint a second identity for
        // the same mailbox: the id lowercases both.
        let a = config_with("Alice@Example.COM", "IMAP.Example.com")
            .account_id()
            .unwrap();
        let b = config_with("alice@example.com", "imap.example.com")
            .account_id()
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn account_id_differs_by_host_for_the_same_username() {
        // The same username on two different servers is two distinct accounts: the host
        // is part of the id, so they never collide in the shared engine store.
        let soverin = config_with("alice@example.com", "imap.soverin.net")
            .account_id()
            .unwrap();
        let fastmail = config_with("alice@example.com", "imap.fastmail.com")
            .account_id()
            .unwrap();
        assert_ne!(soverin, fastmail);
    }
}
