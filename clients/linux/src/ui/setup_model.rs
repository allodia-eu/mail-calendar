//! Account-setup state: what each pane shows, and the conversion to and from the shared
//! autodetection recommendation.

use mailcal_bindings::{
    AccountSetup, ConnectionSecurity, DetectedServerRow, JmapSetup, MissReason,
    SetupRecommendation, account_config_toml, jmap_account_config_toml,
};

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
}

/// What a pane hands the core to connect. Built by whichever pane collected the fields, so the
/// detected and manual routes converge on one conversion.
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
        DetectedForm::Imap(imap) => ManualForm {
            kind: AccountKind::Imap,
            email: imap.email.clone(),
            imap_host: imap.imap_host.clone(),
            smtp_host: imap.smtp_host.clone(),
            caldav_url: imap.caldav_url.clone(),
            ..ManualForm::default()
        },
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
