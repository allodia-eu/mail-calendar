//! Where the calendar and address-book servers found beside a mail server, or instead of one,
//! go on each route.

use mailcal_autodetect::{
    AuthKind, Detected, DetectedDav, DetectedJmap, DetectedMailSettings, DetectedServer,
    SocketKind, Source, SourceKind,
};

use super::{OauthRoutes, SetupRecommendation, recommend};

const EMAIL: &str = "alice@example.com";
const CALDAV: &str = "https://example.com/.well-known/caldav";
const CARDDAV: &str = "https://example.com/.well-known/carddav";

const ALL_ROUTES: OauthRoutes = OauthRoutes {
    google: true,
    microsoft: true,
};

fn both() -> DetectedDav {
    DetectedDav {
        caldav_url: Some(CALDAV.to_owned()),
        carddav_url: Some(CARDDAV.to_owned()),
    }
}

fn source() -> Source {
    Source {
        kind: SourceKind::Autoconfig,
        url: "https://autoconfig.example.com/mail/config-v1.1.xml".to_owned(),
    }
}

fn imap(hostname: &str) -> Detected {
    Detected::Mail(DetectedMailSettings {
        incoming: vec![DetectedServer {
            hostname: hostname.to_owned(),
            port: 993,
            socket: SocketKind::Tls,
            auth: vec![AuthKind::PasswordCleartext],
            username: EMAIL.to_owned(),
        }],
        outgoing: Vec::new(),
        is_trusted: true,
        source: source(),
        dav: both(),
        oauth_issuer: None,
    })
}

#[test]
fn the_imap_route_carries_an_address_book_beside_the_calendar() {
    let SetupRecommendation::Imap {
        caldav_url,
        carddav_url,
        ..
    } = recommend(EMAIL, imap("imap.example.com"), ALL_ROUTES)
    else {
        panic!("expected imap");
    };
    assert_eq!(caldav_url.as_deref(), Some(CALDAV));
    assert_eq!(carddav_url.as_deref(), Some(CARDDAV));
}

#[test]
fn the_jmap_route_carries_what_was_found_beside_it() {
    let detected = Detected::Jmap(DetectedJmap {
        base_url: "https://example.com".to_owned(),
        is_trusted: true,
        source: source(),
        dav: both(),
    });
    let SetupRecommendation::Jmap {
        caldav_url,
        carddav_url,
        ..
    } = recommend(EMAIL, detected, ALL_ROUTES)
    else {
        panic!("expected jmap");
    };
    assert_eq!(caldav_url.as_deref(), Some(CALDAV));
    assert_eq!(carddav_url.as_deref(), Some(CARDDAV));
}

#[test]
fn a_domain_with_only_dav_routes_to_a_calendar_and_contacts_account() {
    assert_eq!(
        recommend(EMAIL, Detected::Dav(both()), ALL_ROUTES),
        SetupRecommendation::Dav {
            email: EMAIL.to_owned(),
            caldav_url: Some(CALDAV.to_owned()),
            carddav_url: Some(CARDDAV.to_owned()),
        }
    );
}

#[test]
fn a_provider_signing_in_through_its_own_api_leaves_what_dav_found_behind() {
    assert_eq!(
        recommend(EMAIL, imap("outlook.office365.com"), ALL_ROUTES),
        SetupRecommendation::Microsoft {
            email: EMAIL.to_owned()
        }
    );
}
