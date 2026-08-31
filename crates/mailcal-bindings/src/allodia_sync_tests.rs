//! What [`super::setup_from_offer`] promises, which is the whole value of syncing an account list.
//!
//! An offer that routes to the manual form has cost the person exactly what account sync was for:
//! they set the account up on one device, and the next one asks them to type its servers in.

use super::{AllodiaAccountKind, AllodiaAccountOffer, setup_from_offer};
use crate::{
    autodetect::{MissReason, SetupRecommendation},
    setup::ConnectionSecurity,
};

const EMAIL: &str = "someone@example.test";

fn offer(kind: AllodiaAccountKind) -> AllodiaAccountOffer {
    AllodiaAccountOffer {
        id: "record".to_owned(),
        email: EMAIL.to_owned(),
        kind,
        host: None,
        port: None,
        security: None,
        smtp_host: None,
        smtp_port: None,
        smtp_security: None,
        caldav_base_url: None,
        jmap_base_url: None,
    }
}

fn imap_offer() -> AllodiaAccountOffer {
    AllodiaAccountOffer {
        host: Some("imap.example.test".to_owned()),
        port: Some(993),
        security: Some(ConnectionSecurity::ImplicitTls),
        smtp_host: Some("smtp.example.test".to_owned()),
        smtp_port: Some(465),
        smtp_security: Some(ConnectionSecurity::ImplicitTls),
        caldav_base_url: Some("https://dav.example.test/".to_owned()),
        ..offer(AllodiaAccountKind::Imap)
    }
}

/// A provider route needs nothing but the address: the sign-in decides the rest.
#[test]
fn a_provider_account_routes_straight_to_its_sign_in() {
    match setup_from_offer(offer(AllodiaAccountKind::Google)) {
        SetupRecommendation::Google { email } => assert_eq!(email, EMAIL),
        other => panic!("a Google offer is a Google sign-in, got {other:?}"),
    }
    match setup_from_offer(offer(AllodiaAccountKind::Microsoft)) {
        SetupRecommendation::Microsoft { email } => assert_eq!(email, EMAIL),
        other => panic!("a Microsoft offer is a Microsoft sign-in, got {other:?}"),
    }
}

/// The servers the other device wrote down are the servers this one takes, so the only thing left
/// to ask for is the password.
#[test]
fn an_imap_offer_carries_every_server_across() {
    match setup_from_offer(imap_offer()) {
        SetupRecommendation::Imap {
            email,
            imap_host,
            smtp_host,
            imap_security,
            smtp_security,
            incoming,
            outgoing,
            caldav_url,
            is_trusted,
            ..
        } => {
            assert_eq!(email, EMAIL);
            assert_eq!(
                imap_host, "imap.example.test",
                "the standard port is implied"
            );
            assert_eq!(smtp_host.as_deref(), Some("smtp.example.test"));
            assert_eq!(imap_security, ConnectionSecurity::ImplicitTls);
            assert_eq!(smtp_security, ConnectionSecurity::ImplicitTls);
            assert_eq!(incoming.hostname, "imap.example.test");
            assert_eq!(incoming.port, 993);
            assert_eq!(incoming.security, "SSL/TLS");
            assert_eq!(incoming.username, EMAIL, "the address is the login");
            assert_eq!(outgoing.expect("submission").hostname, "smtp.example.test");
            assert_eq!(caldav_url.as_deref(), Some("https://dav.example.test/"));
            assert!(
                is_trusted,
                "settings approved on the person's own device are not re-approved here"
            );
        }
        other => panic!("an IMAP offer is an IMAP route, got {other:?}"),
    }
}

/// A non-standard port has to survive, or the form silently connects somewhere else.
#[test]
fn a_non_standard_port_rides_along_in_the_host() {
    let offer = AllodiaAccountOffer {
        port: Some(1993),
        security: Some(ConnectionSecurity::StartTls),
        smtp_port: Some(587),
        smtp_security: Some(ConnectionSecurity::StartTls),
        ..imap_offer()
    };
    match setup_from_offer(offer) {
        SetupRecommendation::Imap {
            imap_host,
            smtp_host,
            incoming,
            ..
        } => {
            assert_eq!(imap_host, "imap.example.test:1993");
            assert_eq!(smtp_host.as_deref(), Some("smtp.example.test:587"));
            assert_eq!(incoming.security, "STARTTLS", "the card says which it is");
        }
        other => panic!("expected an IMAP route, got {other:?}"),
    }
}

/// Reading is routable without sending. An account that can receive and not submit is ordinary,
/// and must not be turned into a manual form over the half it lacks.
#[test]
fn an_account_with_no_submission_still_routes() {
    let offer = AllodiaAccountOffer {
        smtp_host: None,
        smtp_port: None,
        ..imap_offer()
    };
    match setup_from_offer(offer) {
        SetupRecommendation::Imap {
            smtp_host,
            outgoing,
            ..
        } => {
            assert!(smtp_host.is_none());
            assert!(outgoing.is_none(), "no row for a server that is not there");
        }
        other => panic!("expected an IMAP route, got {other:?}"),
    }
}

/// A JMAP record is one URL, and it is the whole route.
#[test]
fn a_jmap_offer_routes_to_the_session_it_names() {
    let offer = AllodiaAccountOffer {
        jmap_base_url: Some("https://jmap.example.test/session".to_owned()),
        ..offer(AllodiaAccountKind::Jmap)
    };
    match setup_from_offer(offer) {
        SetupRecommendation::Jmap {
            email,
            server_url,
            is_trusted,
            ..
        } => {
            assert_eq!(email, EMAIL);
            assert_eq!(server_url, "https://jmap.example.test/session");
            assert!(is_trusted);
        }
        other => panic!("a JMAP offer is a JMAP route, got {other:?}"),
    }
}

/// A record this device cannot route from falls back to detection rather than erroring.
///
/// Detection finds the same server the other device found; what it must not do is present an
/// empty IMAP form as though the offer had filled it.
#[test]
fn a_record_naming_no_server_falls_back_to_detection() {
    for kind in [AllodiaAccountKind::Imap, AllodiaAccountKind::Jmap] {
        match setup_from_offer(offer(kind)) {
            SetupRecommendation::Manual { reason } => {
                assert!(matches!(reason, MissReason::NothingFound));
            }
            other => panic!("{kind:?} with no server must not claim a route, got {other:?}"),
        }
    }
}
