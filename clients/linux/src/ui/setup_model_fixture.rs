//! The routes the tests start from, as a detection run answers them: with the choices the core
//! offers on each.

use mailcal_bindings::{AccountCapability, DetectedSetup, SetupRecommendation};

use super::{SetupForm, detected_form};

/// [`detected_form`] for a test that starts from a route.
pub(in crate::ui) fn recommendation_form(
    recommendation: SetupRecommendation,
    fallback_email: String,
) -> SetupForm {
    detected_form(detected(recommendation), fallback_email)
}

/// What a detection run answers, for a test that starts from a route: the choices the core
/// offers on it.
fn detected(recommendation: SetupRecommendation) -> DetectedSetup {
    let (caldav_url, choices) = match &recommendation {
        SetupRecommendation::Imap { caldav_url, .. } => {
            let found = caldav_url.is_some();
            let choice = |capability, on| mailcal_bindings::SetupChoice {
                capability,
                on,
                server_found: on,
            };
            (
                caldav_url.clone(),
                vec![
                    choice(AccountCapability::Mail, true),
                    choice(AccountCapability::Calendar, found),
                    choice(AccountCapability::Contacts, found),
                ],
            )
        }
        _ => (None, Vec::new()),
    };
    DetectedSetup {
        recommendation,
        calendar_and_contacts: false,
        caldav_url,
        carddav_url: None,
        choices,
    }
}
