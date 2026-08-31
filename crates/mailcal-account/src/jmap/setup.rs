//! The JMAP account-setup form: the fields a host collects, and their serialization into the
//! `[jmap]` config TOML it stores in its OS secure store. Split from the module root to keep
//! both files under the 500-line cap.

use super::JmapAccountConfig;
use crate::{ConfigError, Secret};

/// The fields a host's JMAP account-setup form collects, so first-run setup needs no
/// plaintext seed file: the host passes this to [`build_jmap_config_toml`], stores
/// the result in its OS secure store, then adds it via the app's `add_account`.
///
/// **One** secret field, not two. A JMAP server declares its authentication scheme in
/// the `WWW-Authenticate` challenge of its `401` (RFC 8620 §8.2 specifies none of its
/// own) and the engine's transport negotiates from that: so whether the user pasted a
/// login password, an app-specific password, or an API token no longer changes the wire
/// format, and asking them to pick the right box was only ever a way to get it wrong.
///
/// `server_url` may be left empty, in which case it is derived from the email's domain
/// (`https://<domain>`) for `/.well-known/jmap` discovery.
#[derive(Debug, Clone)]
pub struct JmapSetup {
    /// The account's email address (the Basic-auth username and the account id basis).
    pub email: String,
    /// The JMAP server URL (host, `host:port`, or full URL). Empty ⇒ derive
    /// `https://<email-domain>` and discover the session at `/.well-known/jmap`.
    pub server_url: Option<String>,
    /// The account's secret: a login password, an app-specific password, or an API
    /// token, whichever the server issued. Stored as `password` with the email as
    /// username; see [`build_jmap_config_toml`] for why that form is chosen.
    pub password: String,
}

/// Serializes a [`JmapSetup`] into the `[jmap]` config TOML the host stores in its OS
/// secure store: the JMAP counterpart of [`build_config_toml`](crate::build_config_toml).
/// A bare server host gets an `https://` scheme; an empty server URL is derived from
/// the email domain.
///
/// The secret is always written as **`password`**, never `token`, whatever the user
/// pasted. That is strictly more capable, not merely a naming choice: `password` carries
/// the email as a username, so the engine can present it as `Basic` *or* re-frame it as
/// `Bearer` when a server challenges for one (`Credentials::can_present` in the engine's
/// `provider-jmap`); a bare `token` has no username to build a Basic header from and is
/// therefore bearer-only. Storing the username-bearing form means one stored secret works
/// against a server that changes its mind, or that the user guessed wrong about.
///
/// # Errors
///
/// Returns [`ConfigError::Incomplete`] if the email or the secret is empty, or an empty
/// server URL cannot be derived (the email has no domain), or [`ConfigError::Serialize`]
/// on a serialization error.
pub fn build_jmap_config_toml(setup: &JmapSetup) -> Result<String, ConfigError> {
    let email = setup.email.trim();
    if email.is_empty() {
        return Err(ConfigError::Incomplete("email"));
    }
    let password = setup.password.trim();
    if password.is_empty() {
        return Err(ConfigError::Incomplete("password"));
    }
    let base_url = jmap_base_url(email, setup.server_url.as_deref())?;
    let config = JmapAccountConfig {
        email: email.to_owned(),
        base_url,
        password: Some(Secret::new(password.to_owned())),
        token: None,
        oauth: None,
    };
    config.to_toml()
}

/// Resolves a JMAP account's base URL from what the setup form collected: an explicit
/// `server_url` (a bare host gets `https://`), or (when blank) `https://<email-domain>`,
/// relying on `/.well-known/jmap` autodiscovery.
///
/// Shared by the manual setup path and the OAuth sign-in path, so a discovered account and a
/// hand-entered one can never disagree about which server they mean.
///
/// # Errors
///
/// Returns [`ConfigError::Incomplete`] if the server URL is blank and `email` has no domain
/// to derive one from.
pub fn jmap_base_url(email: &str, server_url: Option<&str>) -> Result<String, ConfigError> {
    let server_url = server_url.map(str::trim).filter(|url| !url.is_empty());
    if let Some(url) = server_url {
        return Ok(normalize_base_url(url));
    }
    let domain = email
        .trim()
        .rsplit_once('@')
        .map(|(_, domain)| domain.trim())
        .filter(|domain| !domain.is_empty())
        .ok_or(ConfigError::Incomplete("server URL"))?;
    Ok(format!("https://{domain}"))
}

/// Ensures a JMAP base URL carries a scheme, defaulting a bare host to `https://`
/// (the JMAP counterpart of the CalDAV base-URL leniency). An explicit scheme is
/// preserved; including `http://` for a local test server.
fn normalize_base_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use provider_jmap::Credentials;

    use super::*;
    use crate::jmap::load_jmap_str;

    fn setup(server_url: Option<&str>, password: &str) -> JmapSetup {
        JmapSetup {
            email: "me@example.net".to_owned(),
            server_url: server_url.map(str::to_owned),
            password: password.to_owned(),
        }
    }

    #[test]
    fn build_toml_with_an_explicit_server_and_password() {
        let config =
            load_jmap_str(&build_jmap_config_toml(&setup(Some("mail.example.net"), "pw")).unwrap())
                .unwrap();
        // A bare host gets an https:// scheme.
        assert_eq!(config.base_url, "https://mail.example.net");
        assert_eq!(config.email, "me@example.net");
        assert_eq!(config.password.unwrap().expose(), "pw");
    }

    #[test]
    fn build_toml_preserves_an_explicit_http_scheme_for_a_local_server() {
        let config = load_jmap_str(
            &build_jmap_config_toml(&setup(Some("http://127.0.0.1:18080"), "pw")).unwrap(),
        )
        .unwrap();
        assert_eq!(config.base_url, "http://127.0.0.1:18080");
    }

    #[test]
    fn build_toml_derives_the_server_from_the_email_domain_when_blank() {
        let config = load_jmap_str(&build_jmap_config_toml(&setup(None, "pw")).unwrap()).unwrap();
        // Empty server URL ⇒ https://<domain> for /.well-known/jmap discovery.
        assert_eq!(config.base_url, "https://example.net");
    }

    #[test]
    fn an_api_token_is_stored_as_a_password_so_it_can_be_presented_under_either_scheme() {
        // The whole point of collapsing the two fields: whatever the user pastes: a login
        // password, an app password, or a Fastmail API token, is stored in the
        // username-bearing form. A bare `token` could only ever go out as Bearer, so a
        // server that challenges `Basic` would reject it; this form works against both.
        let config = load_jmap_str(
            &build_jmap_config_toml(&setup(Some("api.example.net"), "fmapi-token")).unwrap(),
        )
        .unwrap();
        assert_eq!(config.password.as_ref().unwrap().expose(), "fmapi-token");
        assert!(config.token.is_none());
        match config.credentials() {
            Credentials::Basic { username, password } => {
                assert_eq!(username, "me@example.net");
                assert_eq!(password, "fmapi-token");
            }
            Credentials::Bearer(_) => panic!("a setup secret must never be stored bearer-only"),
        }
    }

    #[test]
    fn build_toml_requires_a_secret() {
        assert!(matches!(
            build_jmap_config_toml(&setup(Some("mail.example.net"), "")),
            Err(ConfigError::Incomplete("password"))
        ));
        // A whitespace-only secret counts as absent.
        assert!(matches!(
            build_jmap_config_toml(&setup(Some("mail.example.net"), "  ")),
            Err(ConfigError::Incomplete("password"))
        ));
    }

    #[test]
    fn build_toml_requires_an_email() {
        let mut empty = setup(Some("mail.example.net"), "pw");
        empty.email = "  ".to_owned();
        assert!(matches!(
            build_jmap_config_toml(&empty),
            Err(ConfigError::Incomplete("email"))
        ));
    }

    #[test]
    fn a_config_stored_before_the_collapse_still_loads_and_authenticates() {
        // Backward compatibility: an account set up under the old two-field form persisted
        // `token = …`. It must keep working (as Bearer) rather than silently losing its
        // secret: the collapse changes what we *write*, never what we can *read*.
        let legacy = "[jmap]\nemail = \"me@example.net\"\nbase_url = \"https://api.example.net\"\ntoken = \"legacy-tok\"\n";
        let config = load_jmap_str(legacy).unwrap();
        assert_eq!(config.token.as_ref().unwrap().expose(), "legacy-tok");
        assert!(matches!(config.credentials(), Credentials::Bearer(_)));
    }
}
