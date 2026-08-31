//! Recommendation-mapping tests; one per routing rule.

use mailcal_autodetect::{
    AuthKind, Detected, DetectedJmap, DetectedMailSettings, DetectedServer, SocketKind, Source,
    SourceKind,
};

use super::{ConnectionSecurity, MissReason, OauthRoutes, SetupRecommendation};

const EMAIL: &str = "alice@example.com";

/// A build carrying both OAuth client registrations: the shape of an Allodia build.
const ALL_ROUTES: OauthRoutes = OauthRoutes {
    google: true,
    microsoft: true,
};

/// The routing rules are about what detection *found*, so they all run with both browser
/// sign-ins available. What a build without a registration does instead has its own tests at the
/// end of this file.
fn recommend(email: &str, detected: Detected) -> SetupRecommendation {
    super::recommend(email, detected, ALL_ROUTES)
}

fn server(hostname: &str, port: u16, socket: SocketKind, auth: Vec<AuthKind>) -> DetectedServer {
    DetectedServer {
        hostname: hostname.to_owned(),
        port,
        socket,
        auth,
        username: EMAIL.to_owned(),
    }
}

fn source() -> Source {
    Source {
        kind: SourceKind::Autoconfig,
        url: "https://autoconfig.example.com/mail/config-v1.1.xml".to_owned(),
    }
}

fn mail(incoming: Vec<DetectedServer>, outgoing: Vec<DetectedServer>) -> Detected {
    mail_with_caldav(incoming, outgoing, None)
}

fn mail_with_caldav(
    incoming: Vec<DetectedServer>,
    outgoing: Vec<DetectedServer>,
    caldav_url: Option<String>,
) -> Detected {
    Detected::Mail(DetectedMailSettings {
        incoming,
        outgoing,
        is_trusted: true,
        source: source(),
        caldav_url,
    })
}

fn tls_password(hostname: &str, port: u16) -> DetectedServer {
    server(
        hostname,
        port,
        SocketKind::Tls,
        vec![AuthKind::PasswordCleartext],
    )
}

#[test]
fn jmap_routes_to_the_jmap_form() {
    let detected = Detected::Jmap(DetectedJmap {
        base_url: "https://example.com".to_owned(),
        is_trusted: true,
        source: source(),
    });
    let SetupRecommendation::Jmap {
        email,
        server_url,
        is_trusted,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected jmap");
    };
    assert_eq!(email, EMAIL);
    assert_eq!(server_url, "https://example.com");
    assert!(is_trusted);
}

#[test]
fn a_microsoft_family_host_routes_to_microsoft_even_with_password_auth() {
    // ISPDB may still list Basic auth for Microsoft; the family check wins regardless.
    let detected = mail(
        vec![tls_password("outlook.office365.com", 993)],
        vec![tls_password("smtp.office365.com", 465)],
    );
    assert_eq!(
        recommend(EMAIL, detected),
        SetupRecommendation::Microsoft {
            email: EMAIL.to_owned()
        }
    );
}

#[test]
fn a_business_office365_subdomain_is_microsoft() {
    let detected = mail(
        vec![server(
            "acme.mail.protection.outlook.com",
            993,
            SocketKind::Tls,
            vec![AuthKind::OAuth2],
        )],
        vec![tls_password("smtp.acme.example", 465)],
    );
    assert!(matches!(
        recommend(EMAIL, detected),
        SetupRecommendation::Microsoft { .. }
    ));
}

#[test]
fn a_google_workspace_host_routes_to_the_native_google_sign_in() {
    // A Workspace custom domain (example.com) whose incoming host is Google's; we now have a
    // native Gmail/Calendar integration, so it routes to the Google browser sign-in rather
    // than the IMAP app-password ISPDB would otherwise prefill.
    let detected = mail(
        vec![server(
            "imap.gmail.com",
            993,
            SocketKind::Tls,
            vec![AuthKind::OAuth2, AuthKind::PasswordCleartext],
        )],
        vec![server(
            "smtp.gmail.com",
            465,
            SocketKind::Tls,
            vec![AuthKind::OAuth2, AuthKind::PasswordCleartext],
        )],
    );
    assert_eq!(
        recommend(EMAIL, detected),
        SetupRecommendation::Google {
            email: EMAIL.to_owned()
        }
    );
}

#[test]
fn a_consumer_gmail_address_routes_to_google_without_detection() {
    // gmail.com/googlemail.com route to the native sign-in from the typed domain alone; even
    // when detection turned up nothing (offline) or would have prefilled an IMAP app-password.
    for address in ["bob@gmail.com", "bob@GoogleMail.com"] {
        assert_eq!(
            recommend(
                address,
                Detected::Nothing {
                    network_error: false
                }
            ),
            SetupRecommendation::Google {
                email: address.to_owned()
            }
        );
    }
}

#[test]
fn implicit_tls_servers_carry_implicit_tls_security() {
    let detected = mail(
        vec![tls_password("imap.example.com", 993)],
        vec![tls_password("smtp.example.com", 465)],
    );
    let SetupRecommendation::Imap {
        imap_security,
        smtp_security,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected imap");
    };
    assert_eq!(imap_security, ConnectionSecurity::ImplicitTls);
    assert_eq!(smtp_security, ConnectionSecurity::ImplicitTls);
}

#[test]
fn a_discovered_caldav_endpoint_rides_along_on_the_imap_route() {
    // The follow-on probe found a calendar; the recommendation carries it so the client
    // can offer calendar sync pre-selected (opt-out).
    let detected = mail_with_caldav(
        vec![tls_password("imap.soverin.net", 993)],
        vec![tls_password("smtp.soverin.net", 465)],
        Some("https://caldav.soverin.net/calendars".to_owned()),
    );
    let SetupRecommendation::Imap { caldav_url, .. } = recommend(EMAIL, detected) else {
        panic!("expected imap");
    };
    assert_eq!(
        caldav_url.as_deref(),
        Some("https://caldav.soverin.net/calendars")
    );
}

#[test]
fn no_caldav_leaves_the_imap_route_calendarless() {
    let detected = mail(
        vec![tls_password("imap.example.com", 993)],
        vec![tls_password("smtp.example.com", 465)],
    );
    let SetupRecommendation::Imap { caldav_url, .. } = recommend(EMAIL, detected) else {
        panic!("expected imap");
    };
    assert_eq!(caldav_url, None);
}

#[test]
fn a_non_standard_port_is_kept_in_the_host_field() {
    let detected = mail(
        vec![tls_password("imap.example.com", 1993)],
        vec![tls_password("smtp.example.com", 2465)],
    );
    let SetupRecommendation::Imap {
        imap_host,
        smtp_host,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected imap");
    };
    assert_eq!(imap_host, "imap.example.com:1993");
    assert_eq!(smtp_host.as_deref(), Some("smtp.example.com:2465"));
}

#[test]
fn a_starttls_outgoing_is_configured_as_starttls() {
    // The engine now speaks STARTTLS submission (587), so a STARTTLS-only outgoing is
    // configured (not dropped), on its standard port and labelled STARTTLS.
    let detected = mail(
        vec![tls_password("imap.example.com", 993)],
        vec![server(
            "smtp.example.com",
            587,
            SocketKind::StartTls,
            vec![AuthKind::PasswordCleartext],
        )],
    );
    let SetupRecommendation::Imap {
        smtp_host,
        smtp_security,
        outgoing,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected imap");
    };
    // 587 is the standard STARTTLS submission port, so the host field stays bare.
    assert_eq!(smtp_host.as_deref(), Some("smtp.example.com"));
    assert_eq!(smtp_security, ConnectionSecurity::StartTls);
    assert_eq!(outgoing.unwrap().security, "STARTTLS");
}

#[test]
fn a_starttls_incoming_routes_to_imap_with_starttls() {
    // The engine now speaks STARTTLS (143), so a STARTTLS incoming routes to the IMAP
    // form with STARTTLS carried through, rather than falling back to manual.
    let detected = mail(
        vec![server(
            "imap.example.com",
            143,
            SocketKind::StartTls,
            vec![AuthKind::PasswordCleartext],
        )],
        vec![tls_password("smtp.example.com", 465)],
    );
    let SetupRecommendation::Imap {
        imap_host,
        imap_security,
        smtp_security,
        incoming,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected imap");
    };
    // 143 is the standard STARTTLS IMAP port, so the host field stays bare.
    assert_eq!(imap_host, "imap.example.com");
    assert_eq!(imap_security, ConnectionSecurity::StartTls);
    assert_eq!(incoming.security, "STARTTLS");
    // Per-transport: an implicit-TLS outgoing keeps its own security.
    assert_eq!(smtp_security, ConnectionSecurity::ImplicitTls);
}

#[test]
fn a_non_standard_starttls_port_is_kept_in_the_host_field() {
    // A STARTTLS server on a non-standard port keeps the explicit port, measured against
    // the STARTTLS default (143/587), not the implicit-TLS default (993/465).
    let detected = mail(
        vec![server(
            "imap.example.com",
            1143,
            SocketKind::StartTls,
            vec![AuthKind::PasswordCleartext],
        )],
        vec![server(
            "smtp.example.com",
            587,
            SocketKind::StartTls,
            vec![AuthKind::PasswordCleartext],
        )],
    );
    let SetupRecommendation::Imap {
        imap_host,
        smtp_host,
        ..
    } = recommend(EMAIL, detected)
    else {
        panic!("expected imap");
    };
    assert_eq!(imap_host, "imap.example.com:1143");
    // 587 is standard for STARTTLS submission, so it stays bare.
    assert_eq!(smtp_host.as_deref(), Some("smtp.example.com"));
}

#[test]
fn an_oauth_only_non_microsoft_provider_routes_to_manual() {
    let detected = mail(
        vec![server(
            "imap.example.com",
            993,
            SocketKind::Tls,
            vec![AuthKind::OAuth2],
        )],
        vec![tls_password("smtp.example.com", 465)],
    );
    assert_eq!(
        recommend(EMAIL, detected),
        SetupRecommendation::Manual {
            reason: MissReason::OauthOnlyProvider
        }
    );
}

#[test]
fn nothing_found_distinguishes_offline_from_empty() {
    assert_eq!(
        recommend(
            EMAIL,
            Detected::Nothing {
                network_error: true
            }
        ),
        SetupRecommendation::Manual {
            reason: MissReason::NetworkError
        }
    );
    assert_eq!(
        recommend(
            EMAIL,
            Detected::Nothing {
                network_error: false
            }
        ),
        SetupRecommendation::Manual {
            reason: MissReason::NothingFound
        }
    );
}

#[test]
fn without_googles_registration_a_gmail_address_takes_the_app_password_route() {
    // Gmail's IMAP still accepts an app password, so a build that cannot start the browser
    // sign-in has a route that works; detection's own answer, not a dead end.
    let routes = OauthRoutes {
        google: false,
        microsoft: true,
    };
    let detected = mail(
        vec![tls_password("imap.gmail.com", 993)],
        vec![tls_password("smtp.gmail.com", 465)],
    );
    let SetupRecommendation::Imap { imap_host, .. } =
        super::recommend("bob@gmail.com", detected, routes)
    else {
        panic!("expected the IMAP form");
    };
    assert_eq!(imap_host, "imap.gmail.com");
}

#[test]
fn without_googles_registration_a_workspace_domain_takes_the_app_password_route() {
    let routes = OauthRoutes {
        google: false,
        microsoft: true,
    };
    let detected = mail(
        vec![tls_password("imap.gmail.com", 993)],
        vec![tls_password("smtp.gmail.com", 465)],
    );
    assert!(matches!(
        super::recommend(EMAIL, detected, routes),
        SetupRecommendation::Imap { .. }
    ));
}

#[test]
fn without_microsofts_registration_a_microsoft_domain_says_it_is_oauth_only() {
    // Microsoft retired Basic auth, so the IMAP settings ISPDB still lists cannot log in. A
    // build that cannot start the browser sign-in has nothing to offer, and says which kind of
    // nothing rather than prefilling a form that fails at the password.
    let routes = OauthRoutes {
        google: true,
        microsoft: false,
    };
    let detected = mail(
        vec![tls_password("outlook.office365.com", 993)],
        vec![tls_password("smtp.office365.com", 465)],
    );
    assert_eq!(
        super::recommend(EMAIL, detected, routes),
        SetupRecommendation::Manual {
            reason: MissReason::OauthOnlyProvider
        }
    );
}
