//! Which of mail, calendar and contacts a setup route offers, and how each starts: decided once
//! here, so four clients drawing the found card do not decide it four ways.

use crate::{Capabilities, Capability, ConfigError, SetupRecommendation};

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
            Self::Microsoft { email } => microsoft_choices(email),
            Self::Google { email } => google_choices(email),
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

/// The uses a Microsoft sign-in for `email` offers: every one, all on, and colleagues, shared
/// mailboxes and Teams meetings only when the address is not a personal one.
#[must_use]
pub fn microsoft_choices(email: &str) -> Vec<SetupChoice> {
    provider_choices(is_microsoft_consumer_domain(email), true, true)
}

/// The uses a Google sign-in for `email` offers, as [`microsoft_choices`] does for Microsoft, less
/// shared mailboxes, which a Gmail sign-in cannot open, and video meetings, which this app makes
/// through no Google account.
#[must_use]
pub fn google_choices(email: &str) -> Vec<SetupChoice> {
    provider_choices(
        crate::autodetect::is_google_consumer_domain(email),
        false,
        false,
    )
}

/// Every use, all on; colleagues, shared mailboxes and video meetings left out for a personal
/// account, which has no organisation to have any of them (`docs/accounts.md` rules 3 and 19).
fn provider_choices(personal: bool, shared_mailboxes: bool, meetings: bool) -> Vec<SetupChoice> {
    Capability::ALL
        .into_iter()
        .filter(|capability| match capability {
            Capability::Colleagues => !personal,
            Capability::SharedMailboxes => shared_mailboxes && !personal,
            Capability::Meetings => meetings && !personal,
            _ => true,
        })
        .map(|capability| found(capability, true))
        .collect()
}

/// Microsoft's consumer brands, each under many country domains (`hotmail.co.uk`, `live.nl`).
const MICROSOFT_CONSUMER_BRANDS: &[&str] = &["outlook", "hotmail", "live", "msn", "windowslive"];

/// Whether `email` is at one of Microsoft's personal-account domains: a brand followed by a
/// top-level domain (`live.nl`), or by `co` or `com` and a country (`hotmail.co.uk`,
/// `live.com.au`), so `outlook.example.com` and `live.ing.nl` are not one.
fn is_microsoft_consumer_domain(email: &str) -> bool {
    let Some(domain) = email
        .rsplit_once('@')
        .map(|(_, domain)| domain.to_ascii_lowercase())
    else {
        return false;
    };
    let mut labels = domain.split('.');
    let brand = labels.next().unwrap_or_default();
    let suffix: Vec<&str> = labels.collect();
    MICROSOFT_CONSUMER_BRANDS.contains(&brand)
        && (1..=2).contains(&suffix.len())
        && (suffix.len() == 1 || matches!(suffix[0], "co" | "com") && suffix[1].len() == 2)
}

/// Refuses a choice of nothing, of colleagues or video meetings, or of a use whose server is
/// missing.
pub(crate) fn check_uses(
    uses: &Capabilities,
    caldav: bool,
    carddav: bool,
) -> Result<(), ConfigError> {
    if uses.contains(Capability::Colleagues) {
        return Err(ConfigError::Refused(
            "colleagues come from a Microsoft or Google directory",
        ));
    }
    if uses.contains(Capability::SharedMailboxes) {
        return Err(ConfigError::Refused(
            "a standards account lists its shared mailboxes without being asked for them",
        ));
    }
    if uses.contains(Capability::Meetings) {
        return Err(ConfigError::Refused(
            "video meetings are switched on from the account's page",
        ));
    }
    if uses.iter().next().is_none() {
        return Err(ConfigError::Incomplete("what the account is used for"));
    }
    if uses.contains(Capability::Calendar) && !caldav {
        return Err(ConfigError::Incomplete("calendar server"));
    }
    if uses.contains(Capability::Contacts) && !caldav && !carddav {
        return Err(ConfigError::Incomplete("address book server"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "setup_choices_tests.rs"]
mod tests;
