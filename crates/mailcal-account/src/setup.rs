//! The account-setup builder; turning the fields a host collects in its setup UI into
//! the account-config TOML it stores, the inverse of [`crate::load_str`].
//!
//! Servers are given as a bare host; the standard secure port is assumed and the TLS
//! server name is derived from the host, so users never type ports or a separate "server
//! name". Credentials go straight into the stored TOML; never a plaintext seed file.

use crate::{ConfigError, ConnectionSecurity};

/// The fields a host collects in its account-setup UI, so it can build a config
/// without a plaintext seed file: the host serializes this with [`build_config_toml`]
/// and stores the result in its OS secure store.
///
/// Servers are given as a bare host (e.g. `imap.soverin.net`); the standard secure
/// port for the chosen security is assumed and the TLS server name is the host, so users
/// never type ports or a separate "server name". A power user may still pass an explicit
/// `host:port`.
#[derive(Debug, Clone)]
pub struct AccountSetup {
    /// IMAP mail server: a host (`imap.example.net`) or `host:port`.
    pub imap_host: String,
    /// Login username (the full email address).
    pub username: String,
    /// Login password (or app-specific password).
    pub password: String,
    /// SMTP server (host or `host:port`), if mail-send is configured.
    pub smtp_host: Option<String>,
    /// CalDAV base URL, if calendar sync is configured.
    pub caldav_base_url: Option<String>,
    /// How the IMAP connection is secured; picks the default port (993 vs 143) and the
    /// engine's connect path. Defaults to implicit TLS.
    pub imap_security: ConnectionSecurity,
    /// How the SMTP submission connection is secured; picks the default port (465 vs 587).
    /// Defaults to implicit TLS.
    pub smtp_security: ConnectionSecurity,
}

/// The standard implicit-TLS IMAP port, assumed when a host gives none.
const DEFAULT_IMAP_TLS_PORT: u16 = 993;
/// The standard STARTTLS IMAP port, assumed when a STARTTLS host gives none.
const DEFAULT_IMAP_STARTTLS_PORT: u16 = 143;
/// The standard implicit-TLS SMTP submission port, assumed when a host gives none.
const DEFAULT_SMTP_TLS_PORT: u16 = 465;
/// The standard STARTTLS SMTP submission port, assumed when a STARTTLS host gives none.
const DEFAULT_SMTP_STARTTLS_PORT: u16 = 587;

/// The standard IMAP port for a given connection security.
const fn imap_default_port(security: ConnectionSecurity) -> u16 {
    match security {
        ConnectionSecurity::ImplicitTls => DEFAULT_IMAP_TLS_PORT,
        ConnectionSecurity::StartTls => DEFAULT_IMAP_STARTTLS_PORT,
    }
}

/// The standard SMTP submission port for a given connection security.
const fn smtp_default_port(security: ConnectionSecurity) -> u16 {
    match security {
        ConnectionSecurity::ImplicitTls => DEFAULT_SMTP_TLS_PORT,
        ConnectionSecurity::StartTls => DEFAULT_SMTP_STARTTLS_PORT,
    }
}

/// Splits a user-entered server into its TLS server name (the host) and dial address
/// (`host:port`), applying `default_port` when the input carries none: so a user can
/// type just `imap.example.net` and never needs to know server ports. An explicit
/// `host:port` is preserved.
fn host_and_addr(input: &str, default_port: u16) -> (String, String) {
    if let Some((host, port)) = input.rsplit_once(':')
        && !host.is_empty()
        && !port.is_empty()
        && port.bytes().all(|b| b.is_ascii_digit())
    {
        return (host.to_owned(), input.to_owned());
    }
    (input.to_owned(), format!("{input}:{default_port}"))
}

/// Writes the `security` key into a server section, but only for STARTTLS; implicit TLS
/// is the config's default (`#[serde(default)]`), so omitting the key keeps a standard
/// account's TOML unchanged and round-trips to the same [`ConnectionSecurity`].
fn insert_security(section: &mut toml::Table, security: ConnectionSecurity) {
    if security == ConnectionSecurity::StartTls {
        section.insert("security".into(), "starttls".into());
    }
}

/// Ensures a CalDAV base URL carries a scheme, defaulting to `https://` when the user typed
/// a bare host (e.g. `caldav.soverin.net`): the calendar counterpart of the default-port
/// assumption for IMAP/SMTP, so a user never needs to type `https://`. An explicit scheme
/// (`http://`, `https://`) is preserved.
pub(crate) fn normalize_caldav_base_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    }
}

/// Serializes [`AccountSetup`] into an account-config TOML string: the inverse of
/// [`load_str`](crate::load_str). Servers without an explicit port get the standard secure
/// port and the TLS server name is derived from the host. The `smtp`/`caldav` sections are
/// emitted only when supplied; CalDAV reuses the IMAP username/password (the common
/// single-credential case). A host stores the result in its OS secure store, then passes
/// it to its `new_account` entry point: so credentials never touch a plaintext file.
///
/// # Errors
///
/// Returns [`ConfigError::Incomplete`] if a required field (mail server, username,
/// password) is empty, or [`ConfigError::Serialize`] on a serialization error.
pub fn build_config_toml(setup: &AccountSetup) -> Result<String, ConfigError> {
    if setup.imap_host.trim().is_empty() {
        return Err(ConfigError::Incomplete("mail server"));
    }
    if setup.username.trim().is_empty() {
        return Err(ConfigError::Incomplete("username"));
    }
    if setup.password.is_empty() {
        return Err(ConfigError::Incomplete("password"));
    }

    let username = setup.username.trim();
    let (imap_name, imap_addr) = host_and_addr(
        setup.imap_host.trim(),
        imap_default_port(setup.imap_security),
    );
    let mut imap = toml::Table::new();
    imap.insert("addr".into(), imap_addr.into());
    imap.insert("server_name".into(), imap_name.into());
    imap.insert("username".into(), username.into());
    imap.insert("password".into(), setup.password.clone().into());
    insert_security(&mut imap, setup.imap_security);

    let mut root = toml::Table::new();
    root.insert("imap".into(), imap.into());

    if let Some(host) = &setup.smtp_host
        && !host.trim().is_empty()
    {
        let (name, addr) = host_and_addr(host.trim(), smtp_default_port(setup.smtp_security));
        let mut smtp = toml::Table::new();
        smtp.insert("addr".into(), addr.into());
        smtp.insert("server_name".into(), name.into());
        insert_security(&mut smtp, setup.smtp_security);
        root.insert("smtp".into(), smtp.into());
    }
    if let Some(url) = &setup.caldav_base_url
        && !url.trim().is_empty()
    {
        let mut caldav = toml::Table::new();
        caldav.insert("base_url".into(), normalize_caldav_base_url(url).into());
        caldav.insert("username".into(), username.into());
        caldav.insert("password".into(), setup.password.clone().into());
        root.insert("caldav".into(), caldav.into());
    }
    Ok(toml::to_string(&root)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_str;

    fn full_setup() -> AccountSetup {
        AccountSetup {
            // Bare hosts, no ports: the builder fills in the standard secure ports.
            imap_host: "imap.example.net".to_owned(),
            username: "me@example.net".to_owned(),
            // A password with TOML-special characters exercises the serializer's escaping.
            password: "p@ss\"with'quotes\\and=signs".to_owned(),
            smtp_host: Some("smtp.example.net".to_owned()),
            caldav_base_url: Some("https://dav.example.net".to_owned()),
            imap_security: ConnectionSecurity::ImplicitTls,
            smtp_security: ConnectionSecurity::ImplicitTls,
        }
    }

    #[test]
    fn build_config_toml_defaults_ports_and_derives_server_names() {
        let config = load_str(&build_config_toml(&full_setup()).unwrap()).unwrap();
        // A bare host gets the standard secure port; the TLS server name is the host.
        assert_eq!(config.imap.addr, "imap.example.net:993");
        assert_eq!(config.imap.server_name, "imap.example.net");
        let smtp = config.smtp.unwrap();
        assert_eq!(smtp.addr, "smtp.example.net:465");
        assert_eq!(smtp.server_name, "smtp.example.net");
    }

    #[test]
    fn build_config_toml_defaults_caldav_scheme_to_https() {
        // A bare CalDAV host (no scheme), as a user types it, gets https:// prepended;
        // the calendar counterpart of the default-port leniency for IMAP/SMTP.
        let mut bare = full_setup();
        bare.caldav_base_url = Some("caldav.example.net".to_owned());
        let config = load_str(&build_config_toml(&bare).unwrap()).unwrap();
        assert_eq!(
            config.caldav.unwrap().base_url,
            "https://caldav.example.net"
        );

        // An explicit scheme (and path) is preserved.
        let mut explicit = full_setup();
        explicit.caldav_base_url = Some("http://dav.example.net/cal".to_owned());
        let config = load_str(&build_config_toml(&explicit).unwrap()).unwrap();
        assert_eq!(
            config.caldav.unwrap().base_url,
            "http://dav.example.net/cal"
        );
    }

    #[test]
    fn build_config_toml_uses_starttls_ports_and_records_security() {
        // A STARTTLS account with bare hosts gets the STARTTLS standard ports (143/587),
        // and the security round-trips through load_str so imap_config picks the STARTTLS
        // engine builders.
        let mut setup = full_setup();
        setup.imap_security = ConnectionSecurity::StartTls;
        setup.smtp_security = ConnectionSecurity::StartTls;
        let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
        assert_eq!(config.imap.addr, "imap.example.net:143");
        assert_eq!(config.imap.security, ConnectionSecurity::StartTls);
        let smtp = config.smtp.unwrap();
        assert_eq!(smtp.addr, "smtp.example.net:587");
        assert_eq!(smtp.security, ConnectionSecurity::StartTls);
    }

    #[test]
    fn build_config_toml_omits_security_for_implicit_tls() {
        // The default (implicit TLS) writes no `security` key, so a standard account's TOML
        // is byte-for-byte what it was before the field existed.
        let toml = build_config_toml(&full_setup()).unwrap();
        assert!(!toml.contains("security"));
    }

    #[test]
    fn build_config_toml_preserves_an_explicit_port() {
        let mut setup = full_setup();
        setup.imap_host = "imap.example.net:1993".to_owned();
        let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
        assert_eq!(config.imap.addr, "imap.example.net:1993");
        assert_eq!(config.imap.server_name, "imap.example.net");
    }

    #[test]
    fn build_config_toml_round_trips_credentials_through_load_str() {
        let config = load_str(&build_config_toml(&full_setup()).unwrap()).unwrap();
        assert_eq!(config.imap.username, "me@example.net");
        // The password survives TOML escaping exactly (no plaintext seed file needed).
        assert_eq!(
            config.imap.password.as_ref().unwrap().expose(),
            "p@ss\"with'quotes\\and=signs"
        );
        let caldav = config.caldav.unwrap();
        assert_eq!(caldav.base_url, "https://dav.example.net");
        // CalDAV reuses the IMAP credentials.
        assert_eq!(caldav.username, "me@example.net");
        assert_eq!(
            caldav.password.as_ref().unwrap().expose(),
            "p@ss\"with'quotes\\and=signs"
        );
    }

    #[test]
    fn build_config_toml_omits_unconfigured_optional_sections() {
        let mut setup = full_setup();
        setup.smtp_host = None;
        setup.caldav_base_url = None;
        let config = load_str(&build_config_toml(&setup).unwrap()).unwrap();
        assert!(config.smtp.is_none());
        assert!(config.caldav.is_none());
    }

    #[test]
    fn build_config_toml_rejects_a_missing_required_field() {
        let mut setup = full_setup();
        setup.password = String::new();
        assert!(matches!(
            build_config_toml(&setup),
            Err(ConfigError::Incomplete("password"))
        ));
    }
}
