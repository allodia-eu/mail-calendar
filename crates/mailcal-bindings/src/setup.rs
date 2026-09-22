//! The account-setup form types: a host collects these fields and serializes them into
//! the config TOML it stores in its OS secure store, so first-run setup needs no
//! plaintext seed file. Split out of `lib.rs` to keep it under the 500-line limit.

use crate::MailcalError;

/// A server certificate that did not verify, as a client shows it and hands it back.
///
/// Every field is the certificate's own claim, which is exactly what failed to verify:
/// it is here so a person can recognise a server they meant to reach, and nothing else
/// may rest on it. A client that offers to accept it passes this record back unchanged
/// on [`AccountSetup::accepted_certificate`].
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct RejectedCertificate {
    /// The TLS server name that was asked for; the exception is scoped to it.
    pub server_name: String,
    /// The certificate's SHA-256, uppercase and colon-separated, as every tool shows a
    /// fingerprint and as somebody would compare it against their own server.
    pub sha256: String,
    /// The name the certificate claims to be (`CN`).
    pub subject_common_name: Option<String>,
    /// The organisation it claims to belong to (`O`).
    pub subject_organisation: Option<String>,
    /// Who issued it (`CN`). Equal to the subject when it signed itself.
    pub issuer_common_name: Option<String>,
    /// The organisation that issued it (`O`).
    pub issuer_organisation: Option<String>,
    /// When it claims to become valid, in seconds since the Unix epoch; `None` when the
    /// certificate could not be read at all. Formatted by the client, which knows the
    /// reader's locale and zone (`docs/timestamps.md`).
    pub not_before: Option<i64>,
    /// When it claims to expire, in seconds since the Unix epoch.
    pub not_after: Option<i64>,
}

impl From<mailcal_account::RejectedCertificate> for RejectedCertificate {
    fn from(rejected: mailcal_account::RejectedCertificate) -> Self {
        Self {
            server_name: rejected.server_name,
            sha256: rejected.sha256,
            subject_common_name: rejected.subject_common_name,
            subject_organisation: rejected.subject_organisation,
            issuer_common_name: rejected.issuer_common_name,
            issuer_organisation: rejected.issuer_organisation,
            not_before: rejected.not_before,
            not_after: rejected.not_after,
        }
    }
}

/// How a mail connection is secured, mirrored across the FFI. A client passes the value it
/// received from [`SetupRecommendation::Imap`](crate::SetupRecommendation) straight back in
/// [`AccountSetup`] so the engine dials the same way detection found.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionSecurity {
    /// Implicit TLS from the first byte (standard ports 993/465).
    ImplicitTls,
    /// STARTTLS: connect in the clear, then upgrade to TLS before authenticating (standard
    /// ports 143/587).
    StartTls,
}

impl From<ConnectionSecurity> for mailcal_account::ConnectionSecurity {
    fn from(security: ConnectionSecurity) -> Self {
        match security {
            ConnectionSecurity::ImplicitTls => Self::ImplicitTls,
            ConnectionSecurity::StartTls => Self::StartTls,
        }
    }
}

impl From<mailcal_account::ConnectionSecurity> for ConnectionSecurity {
    fn from(security: mailcal_account::ConnectionSecurity) -> Self {
        match security {
            mailcal_account::ConnectionSecurity::ImplicitTls => Self::ImplicitTls,
            mailcal_account::ConnectionSecurity::StartTls => Self::StartTls,
        }
    }
}

/// The fields a host's account-setup form collects, so first-run setup needs no
/// plaintext seed file: the host passes this to [`account_config_toml`], stores the
/// result in its OS secure store, then connects with
/// [`MailcalApp::new_accounts`](crate::MailcalApp::new_accounts).
#[derive(uniffi::Record)]
pub struct AccountSetup {
    /// IMAP mail server: a host (`imap.soverin.net`) or `host:port`. The standard
    /// secure port for the chosen security (993/143) is assumed when none is given, so
    /// users need not type ports.
    pub imap_host: String,
    /// Login username (the full email address).
    pub username: String,
    /// Login password (or app-specific password).
    pub password: String,
    /// SMTP server (host or `host:port`; default port 465/587), if mail-send is configured.
    pub smtp_host: Option<String>,
    /// CalDAV base URL, if calendar sync is configured.
    pub caldav_base_url: Option<String>,
    /// How the IMAP connection is secured. `None` ⇒ implicit TLS (the manual form's
    /// default); the detected path passes the recommendation's `imap_security`.
    #[uniffi(default = None)]
    pub imap_security: Option<ConnectionSecurity>,
    /// How the SMTP connection is secured. `None` ⇒ implicit TLS; the detected path passes
    /// the recommendation's `smtp_security`.
    #[uniffi(default = None)]
    pub smtp_security: Option<ConnectionSecurity>,
    /// The certificate this account's owner accepted after a connect was refused for it:
    /// the [`RejectedCertificate`] from a
    /// [`MailcalError::CertificateRejected`](crate::MailcalError::CertificateRejected),
    /// passed back unchanged. It is stored with the account, so every later connect
    /// carries it and nobody is asked twice.
    #[uniffi(default = None)]
    pub accepted_certificate: Option<RejectedCertificate>,
}

/// Serializes an [`AccountSetup`] (collected in the host's setup form) into the
/// account-config TOML the host stores in its OS secure store and passes to
/// [`MailcalApp::new_accounts`](crate::MailcalApp::new_accounts). CalDAV reuses the IMAP
/// credentials.
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if a required field is empty or serialization fails.
#[uniffi::export]
pub fn account_config_toml(setup: AccountSetup) -> Result<String, MailcalError> {
    // An acceptance that cannot be read is refused here rather than dropped: dropping it
    // would connect again, be refused for the same certificate again, and ask the same
    // question again, which reads as the app ignoring the answer.
    let accepted_certificate = match &setup.accepted_certificate {
        Some(certificate) => Some(
            mailcal_account::CertificateException::accepted(
                &certificate.server_name,
                &certificate.sha256,
            )
            .ok_or_else(|| {
                MailcalError::Config("the accepted certificate is not a fingerprint".to_owned())
            })?,
        ),
        None => None,
    };
    let input = mailcal_account::AccountSetup {
        imap_host: setup.imap_host,
        username: setup.username,
        password: setup.password,
        smtp_host: setup.smtp_host,
        caldav_base_url: setup.caldav_base_url,
        imap_security: setup.imap_security.map(Into::into).unwrap_or_default(),
        smtp_security: setup.smtp_security.map(Into::into).unwrap_or_default(),
        accepted_certificate,
    };
    mailcal_account::build_config_toml(&input).map_err(|err| MailcalError::Config(err.to_string()))
}

/// The fields a host's **JMAP** account-setup form collects. **One** secret field, not
/// two: the engine negotiates the authentication scheme from the server's
/// `WWW-Authenticate` challenge, so a login password, an app-specific password and an
/// API token are all just "the secret" and the client no longer asks the user to
/// classify theirs. `server_url` may be left empty, in which case it is derived from the
/// email's domain (`https://<domain>`) and the session is discovered at
/// `/.well-known/jmap`.
#[derive(uniffi::Record)]
pub struct JmapSetup {
    /// The account's email address (the Basic-auth username and the account id basis).
    pub email: String,
    /// The JMAP server URL (host, `host:port`, or full URL). `None`/empty ⇒ derive
    /// `https://<email-domain>`.
    pub server_url: Option<String>,
    /// The account's secret: a password, app-specific password, or API token. Stored
    /// with the email as username so it can be presented under either scheme.
    pub password: String,
}

/// Serializes a [`JmapSetup`] into the `[jmap]` config TOML the host stores in its OS
/// secure store and adds via [`MailcalApp::add_account`](crate::MailcalApp::add_account).
/// The JMAP counterpart of [`account_config_toml`].
///
/// # Errors
///
/// Returns [`MailcalError::Config`] if the email or the secret is empty, or
/// serialization fails.
#[uniffi::export]
pub fn jmap_account_config_toml(setup: JmapSetup) -> Result<String, MailcalError> {
    let input = mailcal_account::JmapSetup {
        email: setup.email,
        server_url: setup.server_url,
        password: setup.password,
    };
    mailcal_account::build_jmap_config_toml(&input)
        .map_err(|err| MailcalError::Config(err.to_string()))
}
