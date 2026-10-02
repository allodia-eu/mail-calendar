//! The setup step's FFI answer: the route, the servers found beside it and the choices it offers.

use mailcal_account::SetupRecommendation as Route;

use super::{SetupChoice, detected_setup};
use crate::{AccountCapability, MissReason, SetupRecommendation};

const CALDAV: &str = "https://cloud.example/.well-known/caldav";
const CARDDAV: &str = "https://cloud.example/.well-known/carddav";

#[test]
fn a_domain_with_only_dav_is_a_calendar_and_contacts_setup() {
    let setup = detected_setup(Route::Dav {
        email: "alice@cloud.example".to_owned(),
        caldav_url: Some(CALDAV.to_owned()),
        carddav_url: Some(CARDDAV.to_owned()),
    });
    assert!(setup.calendar_and_contacts);
    // A client that does not read the flag sees the manual form it always did.
    assert_eq!(
        setup.recommendation,
        SetupRecommendation::Manual {
            reason: MissReason::NothingFound
        }
    );
    assert_eq!(setup.caldav_url.as_deref(), Some(CALDAV));
    assert_eq!(setup.carddav_url.as_deref(), Some(CARDDAV));
    assert_eq!(
        setup.choices,
        [
            SetupChoice {
                capability: AccountCapability::Calendar,
                on: true,
                server_found: true,
            },
            SetupChoice {
                capability: AccountCapability::Contacts,
                on: true,
                server_found: true,
            },
        ]
    );
}

#[test]
fn a_jmap_route_carries_what_was_found_beside_it_and_no_choices() {
    let setup = detected_setup(Route::Jmap {
        email: "alice@example.com".to_owned(),
        server_url: "https://example.com".to_owned(),
        is_trusted: true,
        source: String::new(),
        caldav_url: None,
        carddav_url: Some(CARDDAV.to_owned()),
    });
    assert!(!setup.calendar_and_contacts);
    assert!(matches!(
        setup.recommendation,
        SetupRecommendation::Jmap { .. }
    ));
    assert_eq!(setup.carddav_url.as_deref(), Some(CARDDAV));
    assert!(setup.choices.is_empty());
}

#[test]
fn a_provider_sign_in_offers_colleagues_too() {
    let setup = detected_setup(Route::Google {
        email: "alice@gmail.com".to_owned(),
    });
    let offered: Vec<_> = setup
        .choices
        .iter()
        .map(|choice| choice.capability)
        .collect();
    assert_eq!(
        offered,
        [
            AccountCapability::Mail,
            AccountCapability::Calendar,
            AccountCapability::Contacts,
            AccountCapability::Colleagues,
        ]
    );
    assert!(setup.choices.iter().all(|choice| choice.on));
}
