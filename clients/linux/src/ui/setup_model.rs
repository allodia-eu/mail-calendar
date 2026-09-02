//! Account-setup state: what each pane shows, and the conversion to and from the shared
//! autodetection recommendation.

use mailcal_bindings::{
    AccountSetup, ConnectionSecurity, DetectedServerRow, ImapAuthOffer, JmapSetup, MissReason,
    RejectedCertificate, SetupRecommendation, account_config_toml, jmap_account_config_toml,
};

use super::setup_server_field::{ServerPair, split_host};
use crate::l10n;

/// What the setup window is showing: a recommendation the user is confirming, or the manual
/// form they reached by choice or because detection found nothing.
#[derive(Clone)]
pub(super) enum SetupForm {
    Detected(DetectedForm),
    Manual(ManualForm),
}

/// A detection result, on the route the shared core picked for it
/// (`docs/account-autodetect.md` → Routing).
#[derive(Clone)]
pub(super) enum DetectedForm {
    Imap(Box<ImapForm>),
    Jmap(JmapForm),
    Microsoft(OAuthForm),
    Google(OAuthForm),
}

/// The account types the manual form can offer, in picker order; the same four every client
/// lists. Which of them this build actually shows is [`AccountKind::offered`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AccountKind {
    #[default]
    Imap,
    Jmap,
    Microsoft,
    Google,
}

impl AccountKind {
    /// Every kind, in picker order.
    const ALL: [Self; 4] = [Self::Imap, Self::Jmap, Self::Microsoft, Self::Google];

    /// The kinds this build offers, in picker order. A browser sign-in needs an OAuth client
    /// registration, which is injected at build time, so a build given none drops the route
    /// rather than showing a button that fails at the provider. The two credential routes are
    /// always there.
    pub(super) fn offered() -> Vec<Self> {
        let routes = mailcal_bindings::oauth_routes();
        Self::ALL
            .into_iter()
            .filter(|kind| match kind {
                Self::Microsoft => routes.microsoft,
                Self::Google => routes.google,
                Self::Imap | Self::Jmap => true,
            })
            .collect()
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Imap => l10n::setup_account_type_password(),
            Self::Jmap => l10n::setup_account_type_jmap(),
            Self::Microsoft => l10n::setup_account_type_microsoft(),
            Self::Google => l10n::setup_account_type_google(),
        }
    }

    pub(super) fn position(self) -> u32 {
        u32::try_from(
            Self::offered()
                .iter()
                .position(|kind| *kind == self)
                .unwrap_or_default(),
        )
        .unwrap_or_default()
    }

    pub(super) fn from_position(position: u32) -> Self {
        usize::try_from(position)
            .ok()
            .and_then(|index| Self::offered().get(index).copied())
            .unwrap_or_default()
    }
}

/// A detected IMAP account: the servers detection found, shown for recognition rather than
/// editing, plus the CalDAV endpoint the follow-on probe discovered (empty when none).
#[derive(Clone)]
pub(crate) struct ImapForm {
    pub(super) email: String,
    pub(super) imap_host: String,
    pub(super) smtp_host: String,
    pub(super) caldav_url: String,
    pub(super) imap_security: ConnectionSecurity,
    pub(super) smtp_security: ConnectionSecurity,
    pub(super) trusted: bool,
    pub(super) incoming: DetectedServer,
    pub(super) outgoing: Option<DetectedServer>,
    /// The issuer the provider's own autoconfig named, passed straight back to the core when
    /// the sign-in pre-flight runs. `None` is the ordinary case.
    pub(super) oauth_issuer: Option<String>,
    /// What the mail server said it accepts, as answered by the core's fail-soft pre-flight.
    pub(super) sign_in: ImapSignIn,
}

/// A server detection found. The FFI record it comes from is not cloneable and the form is,
/// so the fields the card shows are taken across rather than the row itself.
#[derive(Clone)]
pub(crate) struct DetectedServer {
    pub(super) protocol: String,
    pub(super) hostname: String,
    pub(super) port: u16,
    pub(super) security: String,
}

impl From<DetectedServerRow> for DetectedServer {
    fn from(row: DetectedServerRow) -> Self {
        Self {
            protocol: row.protocol,
            hostname: row.hostname,
            port: row.port,
            security: row.security,
        }
    }
}

#[derive(Clone)]
pub(crate) struct JmapForm {
    pub(super) email: String,
    pub(super) server_url: String,
    pub(super) trusted: bool,
    pub(super) sign_in: JmapSignIn,
}

/// A provider whose whole setup is one browser sign-in; Microsoft or Google. There is nothing
/// to fill in but the address the sign-in targets.
#[derive(Clone)]
pub(crate) struct OAuthForm {
    pub(super) email: String,
}

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
    RegistrationNeeded,
    /// No sign-in here: the password form, as it always was.
    Password,
    /// A sign-in was started and did not finish. The password field comes back, because that
    /// is the route left, and the reason is said rather than left to be guessed at.
    Failed,
}

impl ImapSignIn {
    /// Whether the sign-in button belongs on screen.
    pub(super) const fn show_offer(&self) -> bool {
        matches!(self, Self::Offered { .. })
    }

    /// Whether the password field belongs on screen.
    ///
    /// Not while the server is still being asked, and not when it said a password is refused:
    /// on a provider that has switched password auth off, that field is a dead end and the
    /// user would find out only after typing one.
    pub(super) const fn show_password(&self) -> bool {
        match self {
            Self::Checking => false,
            Self::Offered {
                password_also_works,
                ..
            } => *password_also_works,
            Self::RegistrationNeeded | Self::Password | Self::Failed => true,
        }
    }

    /// Whether to explain that this provider admits only pre-registered applications.
    pub(super) const fn explains_registration(&self) -> bool {
        matches!(self, Self::RegistrationNeeded)
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
            ImapAuthOffer::RegistrationNeeded { .. } => Self::RegistrationNeeded,
            ImapAuthOffer::Password => Self::Password,
        }
    }
}

/// Whether this JMAP server's own metadata advertises sign-in, as answered by the core's
/// fail-soft pre-flight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum JmapSignIn {
    #[default]
    Checking,
    Unavailable,
    Offered,
    Failed,
}

impl JmapSignIn {
    pub(super) const fn show_offer(self) -> bool {
        matches!(self, Self::Offered | Self::Failed)
    }

    /// Whether the **detected** card shows its secret field. Not while the pre-flight is still
    /// asking: a field that appears and is then taken away reads as the app changing its mind,
    /// and the answer decides whether it belongs there at all. The manual form always keeps its
    /// fields: see [`super::setup_jmap`].
    pub(super) const fn show_manual(self) -> bool {
        matches!(self, Self::Unavailable | Self::Failed)
    }
}

/// The manual form: an account type the user picks, and the fields that type needs.
#[derive(Clone, Default)]
pub(crate) struct ManualForm {
    pub(super) kind: AccountKind,
    pub(super) email: String,
    pub(super) imap_host: String,
    pub(super) smtp_host: String,
    pub(super) caldav_url: String,
    pub(super) jmap_server: String,
    pub(super) sign_in: JmapSignIn,
    /// Each server's port and connection security. The host fields above hold the name alone;
    /// the port sits beside it, where the user can see and change it.
    pub(super) servers: ServerPair,
    /// What the typed IMAP server said it accepts. The manual pane keeps its password field
    /// throughout (it is already on screen, and rebuilding over a secret being typed would
    /// erase it), so this only ever *adds* a sign-in button or a line of explanation.
    pub(super) imap_sign_in: ImapSignIn,
    /// Why detection sent the user here, when it did.
    pub(super) note: Option<String>,
}

impl ManualForm {
    /// Whether a JMAP sign-in pre-flight is worth running for what is typed now.
    pub(super) fn probes_jmap_sign_in(&self) -> bool {
        self.kind == AccountKind::Jmap
            && self.sign_in == JmapSignIn::Checking
            && !self.email.trim().is_empty()
    }

    /// Whether an IMAP auth pre-flight is worth running for what is typed now.
    ///
    /// A server is required as well as an address: the question is what *that server*
    /// accepts, and there is nothing to dial without one.
    pub(super) fn probes_imap_sign_in(&self) -> bool {
        self.kind == AccountKind::Imap
            && self.imap_sign_in == ImapSignIn::Checking
            && !self.email.trim().is_empty()
            && !self.imap_host.trim().is_empty()
    }
}

/// What a pane hands the core to connect. Built by whichever pane collected the fields, so the
/// detected and manual routes converge on one conversion.
///
/// The IMAP variant is the wider one because it carries a whole server pair plus an accepted
/// certificate. Boxing it would buy nothing: every route to the core puts the enum in a `Box`
/// already (`AppInput::SubmitAccount`), so the value never travels at its own width.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub(crate) enum AccountSubmission {
    Imap(ImapSubmission),
    Jmap(JmapSubmission),
}

#[derive(Clone)]
pub(crate) struct ImapSubmission {
    pub(super) email: String,
    pub(super) imap_host: String,
    pub(super) smtp_host: String,
    pub(super) caldav_url: String,
    pub(super) imap_security: ConnectionSecurity,
    pub(super) smtp_security: ConnectionSecurity,
    pub(super) password: String,
    /// A certificate the person accepted after this pane was refused for it, passed back
    /// exactly as it arrived (`docs/certificate-exceptions.md`). `None` on every setup that
    /// never met one, which is nearly all of them.
    pub(super) accepted_certificate: Option<RejectedCertificate>,
}

/// What a connect that did not succeed had to say. A refused certificate is its own outcome,
/// because it is the one failure a pane can offer a way past.
#[derive(Clone, Default)]
pub(crate) struct ConnectFailure {
    pub(super) message: Option<String>,
    pub(super) certificate: Option<RejectedCertificate>,
}

#[derive(Clone)]
pub(crate) struct JmapSubmission {
    pub(super) email: String,
    pub(super) server_url: String,
    pub(super) password: String,
}

impl AccountSubmission {
    pub(super) fn config_toml(self) -> Result<String, String> {
        match self {
            Self::Imap(form) => account_config_toml(AccountSetup {
                imap_host: form.imap_host,
                username: form.email,
                password: form.password,
                smtp_host: non_empty(form.smtp_host),
                caldav_base_url: non_empty(form.caldav_url),
                imap_security: Some(form.imap_security),
                smtp_security: Some(form.smtp_security),
                accepted_certificate: form.accepted_certificate,
            })
            .map_err(|error| error.to_string()),
            Self::Jmap(form) => jmap_account_config_toml(JmapSetup {
                email: form.email,
                server_url: non_empty(form.server_url),
                password: form.password,
            })
            .map_err(|error| error.to_string()),
        }
    }
}

pub(super) fn recommendation_form(
    recommendation: SetupRecommendation,
    fallback_email: String,
) -> SetupForm {
    match recommendation {
        SetupRecommendation::Jmap {
            email,
            server_url,
            is_trusted,
            ..
        } => SetupForm::Detected(DetectedForm::Jmap(JmapForm {
            email,
            server_url,
            trusted: is_trusted,
            sign_in: JmapSignIn::Checking,
        })),
        SetupRecommendation::Imap {
            email,
            imap_host,
            smtp_host,
            imap_security,
            smtp_security,
            incoming,
            outgoing,
            caldav_url,
            oauth_issuer,
            is_trusted,
            ..
        } => SetupForm::Detected(DetectedForm::Imap(Box::new(ImapForm {
            email,
            imap_host,
            smtp_host: smtp_host.unwrap_or_default(),
            caldav_url: caldav_url.unwrap_or_default(),
            imap_security,
            smtp_security,
            trusted: is_trusted,
            incoming: incoming.into(),
            outgoing: outgoing.map(Into::into),
            oauth_issuer,
            sign_in: ImapSignIn::Checking,
        }))),
        SetupRecommendation::Microsoft { email } => {
            SetupForm::Detected(DetectedForm::Microsoft(OAuthForm { email }))
        }
        SetupRecommendation::Google { email } => {
            SetupForm::Detected(DetectedForm::Google(OAuthForm { email }))
        }
        SetupRecommendation::Manual { reason } => {
            manual_form(fallback_email, Some(miss_reason(reason)))
        }
    }
}

pub(super) fn manual_form(email: String, note: Option<String>) -> SetupForm {
    SetupForm::Manual(ManualForm {
        email,
        note,
        ..ManualForm::default()
    })
}

/// The manual form behind a detected card's "Set up manually": the same route, prefilled with
/// what detection found, so the user edits a discovered config instead of retyping it.
pub(super) fn edit_manually(form: &DetectedForm) -> SetupForm {
    let manual = match form {
        DetectedForm::Imap(imap) => {
            // The detected hosts carry their port inside them; the manual form shows the two
            // apart, so what detection found stays visible rather than turning into a default.
            let mut servers = ServerPair::default();
            servers
                .imap
                .adopt_detected(&imap.imap_host, imap.imap_security);
            servers
                .smtp
                .adopt_detected(&imap.smtp_host, imap.smtp_security);
            ManualForm {
                kind: AccountKind::Imap,
                email: imap.email.clone(),
                imap_host: split_host(&imap.imap_host).0.to_owned(),
                smtp_host: split_host(&imap.smtp_host).0.to_owned(),
                caldav_url: imap.caldav_url.clone(),
                servers,
                // The card already asked this server; the manual pane asks again for whatever
                // the user edits the server to.
                imap_sign_in: imap.sign_in.clone(),
                ..ManualForm::default()
            }
        }
        DetectedForm::Jmap(jmap) => ManualForm {
            kind: AccountKind::Jmap,
            email: jmap.email.clone(),
            jmap_server: jmap.server_url.clone(),
            // The detected card already ran the pre-flight; the manual pane asks again for
            // whatever the user edits the address to.
            sign_in: jmap.sign_in,
            ..ManualForm::default()
        },
        DetectedForm::Microsoft(form) => ManualForm {
            kind: AccountKind::Microsoft,
            email: form.email.clone(),
            ..ManualForm::default()
        },
        DetectedForm::Google(form) => ManualForm {
            kind: AccountKind::Google,
            email: form.email.clone(),
            ..ManualForm::default()
        },
    };
    SetupForm::Manual(manual)
}

fn miss_reason(reason: MissReason) -> String {
    match reason {
        MissReason::InvalidEmail | MissReason::NothingFound => {
            l10n::setup_detect_reason_nothing().to_owned()
        }
        MissReason::NetworkError => l10n::setup_detect_reason_network().to_owned(),
        MissReason::OauthOnlyProvider => l10n::setup_detect_reason_oauth_only().to_owned(),
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
#[path = "setup_model_tests.rs"]
mod tests;
