//! Which of mail, calendar and contacts a setup route offers, and how each starts: decided once
//! here, so four clients drawing the found card do not decide it four ways.

use crate::{Capability, SetupRecommendation};

/// One use a setup route offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupChoice {
    /// The use.
    pub capability: Capability,
    /// Whether it starts switched on: its server was found, or the provider's sign-in covers it.
    pub on: bool,
    /// Whether its server is known. When it is not and the person switches the use on, the
    /// client asks for the server's URL.
    pub server_found: bool,
}

impl SetupRecommendation {
    /// The uses this route offers, in the order a client lists them. Empty for a route whose
    /// uses are not known before sign-in (JMAP, whose session says what it offers) and for the
    /// manual form.
    #[must_use]
    pub fn choices(&self) -> Vec<SetupChoice> {
        match self {
            Self::Imap {
                caldav_url,
                carddav_url,
                ..
            } => standards_choices(true, caldav_url.is_some(), carddav_url.is_some()),
            Self::Dav {
                caldav_url,
                carddav_url,
                ..
            } => standards_choices(false, caldav_url.is_some(), carddav_url.is_some()),
            Self::Microsoft { .. } | Self::Google { .. } => provider_choices(),
            Self::Jmap { .. } | Self::Manual { .. } => Vec::new(),
        }
    }
}

/// A use that starts on exactly when its server is known.
const fn found(capability: Capability, server_found: bool) -> SetupChoice {
    SetupChoice {
        capability,
        on: server_found,
        server_found,
    }
}

/// The uses a standards account offers: mail when it has a mail server, then calendar and
/// contacts, each on when its server is known. Contacts are looked for at the calendar's server
/// when no address book was found.
#[must_use]
pub fn standards_choices(mail: bool, caldav: bool, carddav: bool) -> Vec<SetupChoice> {
    mail.then(|| found(Capability::Mail, true))
        .into_iter()
        .chain([
            found(Capability::Calendar, caldav),
            found(Capability::Contacts, caldav || carddav),
        ])
        .collect()
}

/// The uses a Microsoft or Google sign-in offers: every one, colleagues included, all on.
#[must_use]
pub fn provider_choices() -> Vec<SetupChoice> {
    Capability::ALL
        .into_iter()
        .map(|capability| found(capability, true))
        .collect()
}

#[cfg(test)]
#[path = "setup_choices_tests.rs"]
mod tests;
