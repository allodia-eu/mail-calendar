//! Conversion tests for the detection FFI surface: the account-layer types map onto
//! their FFI mirrors, the resolver adapter bridges correctly, and a pre-flight error
//! folds into a manual route.

use mailcal_autodetect::{
    AuthKind, Detected, DetectedJmap, DetectedMailSettings, DetectedServer, SocketKind, Source,
    SourceKind,
};

use super::{
    CallbackMxResolver, DnsError, MissReason, MxRecord, MxResolution, MxResolver,
    SetupRecommendation, SrvRecord, SrvResolution, showcase_recommendation, to_recommendation,
};

fn source() -> Source {
    Source {
        kind: SourceKind::JmapWellKnown,
        url: "https://example.com/.well-known/jmap".to_owned(),
    }
}

#[test]
fn a_jmap_detection_converts_to_the_ffi_jmap_route() {
    let detected = Detected::Jmap(DetectedJmap {
        base_url: "https://example.com".to_owned(),
        is_trusted: true,
        source: source(),
    });
    let SetupRecommendation::Jmap {
        email, server_url, ..
    } = to_recommendation("a@example.com", Ok(detected))
    else {
        panic!("expected a jmap route");
    };
    assert_eq!(email, "a@example.com");
    assert_eq!(server_url, "https://example.com");
}

#[test]
fn an_invalid_email_error_folds_to_manual() {
    let result = to_recommendation(
        "nonsense",
        Err(mailcal_autodetect::DetectError::InvalidEmail),
    );
    assert!(matches!(
        result,
        SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail
        }
    ));
}

#[test]
fn a_tls_build_error_folds_to_a_network_manual() {
    let result = to_recommendation(
        "a@example.com",
        Err(mailcal_autodetect::DetectError::Tls("boom".to_owned())),
    );
    assert!(matches!(
        result,
        SetupRecommendation::Manual {
            reason: MissReason::NetworkError
        }
    ));
}

#[test]
fn a_discovered_caldav_endpoint_survives_the_ffi_conversion() {
    let detected = Detected::Mail(DetectedMailSettings {
        oauth_issuer: None,
        incoming: vec![DetectedServer {
            hostname: "imap.soverin.net".to_owned(),
            port: 993,
            socket: SocketKind::Tls,
            auth: vec![AuthKind::PasswordCleartext],
            username: "info@example.org".to_owned(),
        }],
        outgoing: Vec::new(),
        is_trusted: true,
        source: source(),
        caldav_url: Some("https://caldav.soverin.net/calendars".to_owned()),
    });
    let SetupRecommendation::Imap { caldav_url, .. } =
        to_recommendation("info@example.org", Ok(detected))
    else {
        panic!("expected an imap route");
    };
    assert_eq!(
        caldav_url.as_deref(),
        Some("https://caldav.soverin.net/calendars")
    );
}

#[test]
fn a_clean_empty_result_folds_to_manual_nothing_found() {
    let result = to_recommendation(
        "a@example.com",
        Ok(Detected::Nothing {
            network_error: false,
        }),
    );
    assert!(matches!(
        result,
        SetupRecommendation::Manual {
            reason: MissReason::NothingFound
        }
    ));
}

/// A fake host resolver returning one authenticated MX record and one SRV record.
struct FakeResolver;

impl MxResolver for FakeResolver {
    fn resolve_mx(&self, _domain: String) -> Result<MxResolution, DnsError> {
        Ok(MxResolution {
            records: vec![MxRecord {
                preference: 10,
                exchange: "mx.example.com".to_owned(),
            }],
            authentic_data: true,
        })
    }

    fn resolve_srv(&self, _name: String) -> Result<SrvResolution, DnsError> {
        Ok(SrvResolution {
            records: vec![SrvRecord {
                priority: 0,
                weight: 1,
                port: 443,
                target: "api.example.com".to_owned(),
            }],
            authentic_data: true,
        })
    }
}

/// A fake host resolver whose lookups fail.
struct FailingResolver;

impl MxResolver for FailingResolver {
    fn resolve_mx(&self, _domain: String) -> Result<MxResolution, DnsError> {
        Err(DnsError::Lookup("no network".to_owned()))
    }

    fn resolve_srv(&self, _name: String) -> Result<SrvResolution, DnsError> {
        Err(DnsError::Lookup("no network".to_owned()))
    }
}

#[test]
fn the_resolver_adapter_bridges_records() {
    use mailcal_autodetect::MxResolver as _;
    let adapter = CallbackMxResolver(Box::new(FakeResolver));
    let resolution = adapter.resolve_mx("example.com").expect("resolves");
    assert!(resolution.authentic_data);
    assert_eq!(resolution.records.len(), 1);
    assert_eq!(resolution.records[0].preference, 10);
    assert_eq!(resolution.records[0].exchange, "mx.example.com");
}

#[test]
fn the_resolver_adapter_bridges_srv_records() {
    use mailcal_autodetect::MxResolver as _;
    let adapter = CallbackMxResolver(Box::new(FakeResolver));
    let resolution = adapter
        .resolve_srv("_jmap._tcp.example.com")
        .expect("resolves");
    assert!(resolution.authentic_data);
    assert_eq!(resolution.records.len(), 1);
    assert_eq!(resolution.records[0].port, 443);
    assert_eq!(resolution.records[0].target, "api.example.com");
}

#[test]
fn the_resolver_adapter_maps_a_failure() {
    use mailcal_autodetect::MxResolver as _;
    let adapter = CallbackMxResolver(Box::new(FailingResolver));
    assert!(adapter.resolve_mx("example.com").is_err());
    assert!(adapter.resolve_srv("_jmap._tcp.example.com").is_err());
}

// ---- the showcase (screenshot) detection script ------------------------------------------------
//
// Detection is a network call, and a showcase build has no network: so the account-setup
// documentation could not be screenshotted at all without these canned answers. What is pinned
// here is that each of the three doc screens is reachable, and (the one that would be a security
// bug rather than a broken picture) that the untrusted route really reports itself untrusted.

#[test]
fn the_showcase_trusted_domain_routes_to_imap_with_a_calendar() {
    let SetupRecommendation::Imap {
        email,
        imap_host,
        smtp_host,
        caldav_url,
        is_trusted,
        source,
        ..
    } = showcase_recommendation("eva@northwind.example")
    else {
        panic!("expected the imap route");
    };
    assert_eq!(email, "eva@northwind.example");
    assert_eq!(imap_host, "imap.northwind.example");
    assert_eq!(smtp_host.as_deref(), Some("smtp.northwind.example"));
    assert!(is_trusted);
    // The calendar toggle is shown pre-checked only when an endpoint was discovered, so this is
    // what the guide's opt-out screenshot depends on.
    assert_eq!(
        caldav_url.as_deref(),
        Some("https://dav.northwind.example/")
    );
    assert!(source.starts_with("autoconfig (https://"));
}

#[test]
fn the_showcase_untrusted_domain_reports_itself_untrusted() {
    // The approval gate is a security contract (docs/account-autodetect.md): a non-HTTPS hop must
    // reach the user as `is_trusted: false`, or the screenshot would picture a warning the app
    // does not actually raise.
    let SetupRecommendation::Imap {
        is_trusted,
        caldav_url,
        source,
        ..
    } = showcase_recommendation("bram@oldschool.example")
    else {
        panic!("expected the imap route");
    };
    assert!(!is_trusted);
    assert!(caldav_url.is_none());
    assert!(source.starts_with("autoconfig (http://"));
}

#[test]
fn every_other_showcase_domain_falls_to_the_manual_form() {
    assert!(matches!(
        showcase_recommendation("eva.jansen@example.com"),
        SetupRecommendation::Manual {
            reason: MissReason::NothingFound
        }
    ));
}

#[test]
fn a_showcase_address_without_a_domain_is_an_invalid_email() {
    assert!(matches!(
        showcase_recommendation("nonsense"),
        SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail
        }
    ));
    assert!(matches!(
        showcase_recommendation("trailing@"),
        SetupRecommendation::Manual {
            reason: MissReason::InvalidEmail
        }
    ));
}

#[test]
fn the_showcase_script_ignores_domain_case() {
    // The setup field is free text; a capture that typed `Northwind.Example` must not silently
    // fall through to the manual route and photograph the wrong screen.
    assert!(matches!(
        showcase_recommendation("Eva@Northwind.Example"),
        SetupRecommendation::Imap { .. }
    ));
}
