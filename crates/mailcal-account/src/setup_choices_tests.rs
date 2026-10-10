//! Which uses each setup route offers, and how each starts.

use super::SetupChoice;
use crate::{Capability, ConnectionSecurity, MissReason, ServerSummary, SetupRecommendation};

const EMAIL: &str = "alice@example.com";
const CALDAV: &str = "https://example.com/.well-known/caldav";
const CARDDAV: &str = "https://example.com/.well-known/carddav";

fn choice(capability: Capability, on: bool, server_found: bool) -> SetupChoice {
    SetupChoice {
        capability,
        on,
        server_found,
    }
}

fn imap(caldav: Option<&str>, carddav: Option<&str>) -> SetupRecommendation {
    let summary = ServerSummary {
        protocol: "IMAP".to_owned(),
        hostname: "imap.example.com".to_owned(),
        port: 993,
        security: "SSL/TLS".to_owned(),
        username: EMAIL.to_owned(),
    };
    SetupRecommendation::Imap {
        email: EMAIL.to_owned(),
        imap_host: "imap.example.com".to_owned(),
        smtp_host: None,
        imap_security: ConnectionSecurity::ImplicitTls,
        smtp_security: ConnectionSecurity::ImplicitTls,
        incoming: summary,
        outgoing: None,
        caldav_url: caldav.map(str::to_owned),
        carddav_url: carddav.map(str::to_owned),
        oauth_issuer: None,
        is_trusted: true,
        source: String::new(),
    }
}

fn dav(caldav: Option<&str>, carddav: Option<&str>) -> SetupRecommendation {
    SetupRecommendation::Dav {
        email: EMAIL.to_owned(),
        caldav_url: caldav.map(str::to_owned),
        carddav_url: carddav.map(str::to_owned),
    }
}

#[test]
fn a_mailbox_with_a_calendar_found_starts_with_all_three_on() {
    assert_eq!(
        imap(Some(CALDAV), None).choices(),
        [
            choice(Capability::Mail, true, true),
            choice(Capability::Calendar, true, true),
            // Contacts are looked for at the calendar's server when no address book was found.
            choice(Capability::Contacts, true, true),
        ]
    );
}

#[test]
fn a_mailbox_alone_offers_calendar_and_contacts_switched_off() {
    assert_eq!(
        imap(None, None).choices(),
        [
            choice(Capability::Mail, true, true),
            choice(Capability::Calendar, false, false),
            choice(Capability::Contacts, false, false),
        ]
    );
}

#[test]
fn an_address_book_found_alone_switches_contacts_on() {
    assert_eq!(
        imap(None, Some(CARDDAV)).choices(),
        [
            choice(Capability::Mail, true, true),
            choice(Capability::Calendar, false, false),
            choice(Capability::Contacts, true, true),
        ]
    );
}

#[test]
fn a_calendar_and_contacts_route_offers_no_mail() {
    assert_eq!(
        dav(Some(CALDAV), Some(CARDDAV)).choices(),
        [
            choice(Capability::Calendar, true, true),
            choice(Capability::Contacts, true, true),
        ]
    );
    assert_eq!(
        dav(None, Some(CARDDAV)).choices(),
        [
            choice(Capability::Calendar, false, false),
            choice(Capability::Contacts, true, true),
        ]
    );
}

/// A personal address has no organisation directory, so colleagues are not offered before a
/// sign-in that could never grant them (`docs/accounts.md` rule 3).
#[test]
fn a_personal_address_is_offered_no_colleagues() {
    let three = [
        choice(Capability::Mail, true, true),
        choice(Capability::Calendar, true, true),
        choice(Capability::Contacts, true, true),
    ];
    for email in ["someone@gmail.com", "someone@googlemail.com"] {
        let route = SetupRecommendation::Google {
            email: email.to_owned(),
        };
        assert_eq!(route.choices(), three, "{email}");
    }
    for email in [
        "someone@outlook.com",
        "someone@hotmail.co.uk",
        "someone@live.com.au",
        "someone@live.nl",
        "someone@msn.com",
        "someone@Outlook.de",
    ] {
        let route = SetupRecommendation::Microsoft {
            email: email.to_owned(),
        };
        assert_eq!(route.choices(), three, "{email}");
    }
    // A domain that only starts like one is an organisation's.
    for email in ["someone@outlook.example.com", "someone@live.ing.nl"] {
        let organisation = SetupRecommendation::Microsoft {
            email: email.to_owned(),
        };
        assert_eq!(organisation.choices().len(), 4, "{email}");
    }
}

#[test]
fn a_provider_sign_in_offers_everything_switched_on() {
    let everything = [
        choice(Capability::Mail, true, true),
        choice(Capability::Calendar, true, true),
        choice(Capability::Contacts, true, true),
        choice(Capability::Colleagues, true, true),
    ];
    for route in [
        SetupRecommendation::Microsoft {
            email: EMAIL.to_owned(),
        },
        SetupRecommendation::Google {
            email: EMAIL.to_owned(),
        },
    ] {
        assert_eq!(route.choices(), everything);
    }
}

/// A JMAP account's session says what it offers, and nothing is known of it before sign-in.
#[test]
fn jmap_and_manual_routes_have_no_choices_before_sign_in() {
    let jmap = SetupRecommendation::Jmap {
        email: EMAIL.to_owned(),
        server_url: "https://example.com".to_owned(),
        is_trusted: true,
        source: String::new(),
        caldav_url: Some(CALDAV.to_owned()),
        carddav_url: None,
    };
    assert!(jmap.choices().is_empty());
    let manual = SetupRecommendation::Manual {
        reason: MissReason::NothingFound,
    };
    assert!(manual.choices().is_empty());
}
