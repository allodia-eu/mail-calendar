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
        let found = |capability, server_found| SetupChoice {
            capability,
            on: server_found,
            server_found,
        };
        match self {
            Self::Imap {
                caldav_url,
                carddav_url,
                ..
            } => vec![
                found(Capability::Mail, true),
                found(Capability::Calendar, caldav_url.is_some()),
                // Contacts are looked for at the calendar's server when no address book was found.
                found(
                    Capability::Contacts,
                    caldav_url.is_some() || carddav_url.is_some(),
                ),
            ],
            Self::Dav {
                caldav_url,
                carddav_url,
                ..
            } => vec![
                found(Capability::Calendar, caldav_url.is_some()),
                found(
                    Capability::Contacts,
                    caldav_url.is_some() || carddav_url.is_some(),
                ),
            ],
            Self::Microsoft { .. } | Self::Google { .. } => Capability::ALL
                .into_iter()
                .map(|capability| found(capability, true))
                .collect(),
            Self::Jmap { .. } | Self::Manual { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "setup_choices_tests.rs"]
mod tests;
