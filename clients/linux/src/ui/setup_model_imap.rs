//! The IMAP half of the setup model: what the server said it accepts, and the manual form as
//! the account shape a pre-flight and a sign-in both take.
//!
//! Split from [`super`], which had reached the size limit. A child module, so it reaches the
//! form types' fields as that module does.

use mailcal_bindings::{ConnectionSecurity, ImapAuthOffer};

use super::{DetectedServer, ImapForm, ManualForm};

/// What an IMAP server said it accepts, as answered by the core's fail-soft pre-flight.
///
/// Three answers rather than a flag, because they are three screens. The middle one is why:
/// a provider that admits only applications registered with it in advance is not the same as
/// one that offers no sign-in, and showing the same bare password form for both leaves
/// someone wondering why the button their colleague has is missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum ImapSignIn {
    /// The server has not answered yet. Nothing is offered and no password field is drawn:
    /// a field that appears and is then taken away reads as the app changing its mind.
    #[default]
    Checking,
    /// Sign-in is on offer.
    Offered {
        /// The provider's name for the button, when this build's registration names one.
        label: Option<String>,
        /// Whether a password still works, so "use a password instead" is worth offering.
        password_also_works: bool,
    },
    /// The provider's sign-in exists but is not open to this application.
    RegistrationNeeded {
        /// Whether a password still works. When it does not, the account cannot be added here.
        password_also_works: bool,
    },
    /// No sign-in here: the password form, as it always was.
    Password,
    /// A sign-in was started and did not finish. The password field comes back, because that
    /// is the route left, and the reason is said rather than left to be guessed at.
    Failed,
}

impl ImapSignIn {
    /// Whether the sign-in button belongs on screen.
    pub(in crate::ui) const fn show_offer(&self) -> bool {
        matches!(self, Self::Offered { .. })
    }

    /// Whether the password field belongs on screen.
    ///
    /// Not while the server is still being asked, and not when it said a password is refused:
    /// on a provider that has switched password auth off, that field is a dead end and the
    /// user would find out only after typing one.
    pub(in crate::ui) const fn show_password(&self) -> bool {
        match self {
            Self::Checking => false,
            Self::Offered {
                password_also_works,
                ..
            }
            | Self::RegistrationNeeded {
                password_also_works,
            } => *password_also_works,
            Self::Password | Self::Failed => true,
        }
    }

    /// Whether the server answered that a password does not work, which takes a password field
    /// already drawn away (`docs/mail-oauth.md` rule 8).
    pub(in crate::ui) const fn refuses_password(&self) -> bool {
        matches!(
            self,
            Self::Offered {
                password_also_works: false,
                ..
            } | Self::RegistrationNeeded {
                password_also_works: false,
            }
        )
    }

    /// Whether to explain that this provider admits only pre-registered applications.
    pub(in crate::ui) const fn explains_registration(&self) -> bool {
        matches!(self, Self::RegistrationNeeded { .. })
    }
}

/// How a detected row names a connection's security.
const fn security_label(security: ConnectionSecurity) -> &'static str {
    match security {
        ConnectionSecurity::ImplicitTls => "SSL/TLS",
        ConnectionSecurity::StartTls => "STARTTLS",
    }
}

impl From<ManualForm> for ImapForm {
    /// The typed fields as the account shape the pre-flight and the sign-in both take.
    ///
    /// The manual form has no detected servers to summarise and no trust question to answer:
    /// nothing was fetched, so there is no untrusted hop to approve, and the rows on the
    /// detected card exist to be recognised rather than retyped. The one thing carried
    /// across is what the user typed.
    fn from(form: ManualForm) -> Self {
        let (imap, smtp) = (&form.servers.imap, &form.servers.smtp);
        Self {
            // `host:port`, the shape a detected server carries its port in.
            imap_host: imap.dial(&form.imap_host),
            smtp_host: smtp.dial(&form.smtp_host),
            imap_security: imap.security(),
            smtp_security: smtp.security(),
            trusted: true,
            incoming: DetectedServer {
                protocol: "IMAP".to_owned(),
                port: imap.port().parse().unwrap_or(993),
                security: security_label(imap.security()).to_owned(),
                hostname: form.imap_host,
            },
            email: form.email,
            caldav_url: form.caldav_url,
            carddav_url: form.carddav_url,
            uses: None,
            offer: super::UseOffer::default(),
            outgoing: None,
            // Nothing was detected, so no provider named an issuer for itself; the core's
            // well-known probe is what answers here.
            oauth_issuer: None,
            sign_in: form.imap_sign_in,
        }
    }
}

impl From<ImapAuthOffer> for ImapSignIn {
    fn from(offer: ImapAuthOffer) -> Self {
        match offer {
            ImapAuthOffer::SignIn {
                provider_label,
                password_also_works,
                // The issuer is shown by the core's own log rather than on the card: a URL
                // beside a button asks the user to make a judgement they have no basis for,
                // and the server it names is the one their provider published.
                ..
            } => Self::Offered {
                label: provider_label,
                password_also_works,
            },
            ImapAuthOffer::RegistrationNeeded {
                password_also_works,
            } => Self::RegistrationNeeded {
                password_also_works,
            },
            ImapAuthOffer::Password => Self::Password,
        }
    }
}
