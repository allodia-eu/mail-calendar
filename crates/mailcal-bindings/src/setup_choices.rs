//! What the found card offers: the detected route together with which of mail, calendar and
//! contacts it can set up, and how each starts.

use crate::{AccountCapability, MailcalApp, MxResolver, SetupRecommendation, autodetect::convert};

/// One use a setup route offers.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct SetupChoice {
    /// The use.
    pub capability: AccountCapability,
    /// Whether its toggle starts switched on: its server was found, or the provider's sign-in
    /// covers it.
    pub on: bool,
    /// Whether its server is known. When it is not and the person switches the use on, the
    /// client asks for the server's URL (CalDAV for calendar, CardDAV for contacts).
    pub server_found: bool,
}

/// A detection run's answer for the setup step.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DetectedSetup {
    /// Where the setup step routes, as [`MailcalApp::detect_account_settings`] answers. A domain
    /// with a calendar or address book but no mail server is
    /// [`SetupRecommendation::Manual`] here, with `calendar_and_contacts` set.
    pub recommendation: SetupRecommendation,
    /// No mail server was found, but a calendar or address-book server was: the client offers an
    /// account used for those alone, the typed address as its login
    /// ([`AccountSetup::uses`](crate::AccountSetup::uses) without mail).
    pub calendar_and_contacts: bool,
    /// The CalDAV endpoint found beside the route's server, or alone.
    pub caldav_url: Option<String>,
    /// The CardDAV endpoint found beside the route's server, or alone; `None` means contacts
    /// are looked for at the calendar's endpoint.
    pub carddav_url: Option<String>,
    /// The uses the route offers, in the order a client lists them. Empty for a JMAP route,
    /// whose session says what it offers once signed in, and for the manual form.
    pub choices: Vec<SetupChoice>,
}

#[uniffi::export]
impl MailcalApp {
    /// [`detect_account_settings`](MailcalApp::detect_account_settings), with the choices the
    /// found card offers and the calendar and address-book servers found beside the route or
    /// instead of one. Blocking and never throwing, as that is.
    pub fn detect_account_setup(
        &self,
        email: String,
        mx_resolver: Option<Box<dyn MxResolver>>,
    ) -> DetectedSetup {
        detected_setup(self.detect_route(&email, mx_resolver))
    }
}

/// The FFI answer for an account-layer route.
pub(crate) fn detected_setup(route: mailcal_account::SetupRecommendation) -> DetectedSetup {
    use mailcal_account::SetupRecommendation as R;
    let choices = route
        .choices()
        .into_iter()
        .map(|choice| SetupChoice {
            capability: choice.capability.into(),
            on: choice.on,
            server_found: choice.server_found,
        })
        .collect();
    let (caldav_url, carddav_url) = match &route {
        R::Imap {
            caldav_url,
            carddav_url,
            ..
        }
        | R::Jmap {
            caldav_url,
            carddav_url,
            ..
        }
        | R::Dav {
            caldav_url,
            carddav_url,
            ..
        } => (caldav_url.clone(), carddav_url.clone()),
        R::Microsoft { .. } | R::Google { .. } | R::Manual { .. } => (None, None),
    };
    DetectedSetup {
        calendar_and_contacts: matches!(route, R::Dav { .. }),
        recommendation: convert(route),
        caldav_url,
        carddav_url,
        choices,
    }
}

#[cfg(test)]
#[path = "setup_choices_tests.rs"]
mod tests;
