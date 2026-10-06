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

/// The uses a Microsoft sign-in for `email` offers: every one, all on, and colleagues only when
/// the address is not a personal one.
#[must_use]
pub fn microsoft_choices(email: &str) -> Vec<SetupChoice> {
    provider_choices(is_microsoft_consumer_domain(email))
}

/// The uses a Google sign-in for `email` offers, as [`microsoft_choices`] does for Microsoft.
#[must_use]
pub fn google_choices(email: &str) -> Vec<SetupChoice> {
    provider_choices(crate::autodetect::is_google_consumer_domain(email))
}

/// Every use, all on; colleagues left out for a personal account, which has no organisation
/// directory (`docs/accounts.md` rule 3).
fn provider_choices(personal: bool) -> Vec<SetupChoice> {
    Capability::ALL
        .into_iter()
        .filter(|capability| !(personal && *capability == Capability::Colleagues))
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

#[cfg(test)]
#[path = "setup_choices_tests.rs"]
mod tests;
