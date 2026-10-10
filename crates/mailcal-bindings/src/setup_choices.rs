//! What the found card offers: the detected route together with which of mail, calendar and
//! contacts it can set up, and how each starts.

use crate::{
    AccountCapability, AccountKind, AllodiaAccountOffer, MailcalApp, MxResolver,
    SetupRecommendation, autodetect::convert,
};

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

/// [`setup_from_offer`](crate::setup_from_offer), as the found card takes it: the route an
/// account from one of the person's other devices opens, with the choices that card offers.
#[uniffi::export]
#[must_use]
pub fn offered_setup(offer: AllodiaAccountOffer) -> DetectedSetup {
    detected_setup(crate::allodia_sync::offer_route(offer))
}

/// The uses a Microsoft or Google sign-in for `email` offers before the browser opens, for a
/// manual form that reached that sign-in without a detection run. Empty for every other kind,
/// whose servers decide.
#[uniffi::export]
#[must_use]
pub fn provider_setup_choices(kind: AccountKind, email: String) -> Vec<SetupChoice> {
    ffi_choices(match kind {
        AccountKind::Microsoft => mailcal_account::microsoft_choices(&email),
        AccountKind::Google => mailcal_account::google_choices(&email),
        AccountKind::Imap | AccountKind::Dav | AccountKind::Jmap => Vec::new(),
    })
}

/// Whether setup ends by offering to link an account of `kind` to another. Only a standards
/// account is offered it: Microsoft, Google and JMAP hold mail, calendar and contacts themselves,
/// so linking one is left to Settings, where it can be done at any time.
#[uniffi::export]
#[must_use]
pub fn offers_setup_links(kind: AccountKind) -> bool {
    matches!(kind, AccountKind::Imap | AccountKind::Dav)
}

fn ffi_choices(choices: Vec<mailcal_account::SetupChoice>) -> Vec<SetupChoice> {
    choices
        .into_iter()
        .map(|choice| SetupChoice {
            capability: choice.capability.into(),
            on: choice.on,
            server_found: choice.server_found,
        })
        .collect()
}

/// The FFI answer for an account-layer route.
pub(crate) fn detected_setup(route: mailcal_account::SetupRecommendation) -> DetectedSetup {
    use mailcal_account::SetupRecommendation as R;
    let choices = ffi_choices(route.choices());
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
