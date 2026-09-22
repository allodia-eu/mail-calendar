//! Route-to-form conversion: which pane a recommendation reaches, and what it hands the core.

use mailcal_bindings::{ConnectionSecurity, SetupRecommendation};

use super::{
    AccountKind, AccountSubmission, DetectedForm, ImapSubmission, JmapSignIn, SetupForm,
    edit_manually, recommendation_form,
};

#[test]
fn every_oauth_provider_routes_to_its_own_sign_in_pane() {
    assert_eq!(
        form_kind(&recommendation_form(
            SetupRecommendation::Google {
                email: "person@gmail.com".to_owned(),
            },
            String::new(),
        )),
        "google"
    );
    // Microsoft used to land on the IMAP/password form with a hint above it, which asked for a
    // password Microsoft no longer accepts.
    assert_eq!(
        form_kind(&recommendation_form(
            SetupRecommendation::Microsoft {
                email: "person@outlook.com".to_owned(),
            },
            String::new(),
        )),
        "microsoft"
    );
}

#[test]
fn detected_jmap_asks_before_it_offers_anything() {
    let SetupForm::Detected(DetectedForm::Jmap(form)) = jmap_form() else {
        panic!("expected JMAP form");
    };

    // Neither surface until the server answers: the card asks first and shows the outcome.
    assert_eq!(form.sign_in, JmapSignIn::Checking);
    assert!(!form.sign_in.show_offer());
    assert!(!form.sign_in.show_manual());
}

#[test]
fn detected_security_and_calendar_ride_back_into_the_config() {
    let SetupForm::Detected(DetectedForm::Imap(form)) = imap_form() else {
        panic!("expected IMAP form");
    };

    let config = AccountSubmission::Imap(ImapSubmission {
        email: form.email,
        imap_host: form.imap_host,
        smtp_host: form.smtp_host,
        caldav_url: form.caldav_url,
        imap_security: form.imap_security,
        smtp_security: form.smtp_security,
        password: "secret".to_owned(),
        accepted_certificate: None,
    })
    .config_toml()
    .expect("valid config");

    assert!(config.contains("starttls"));
    assert!(config.contains("calendar.example.test"));
    // An account that met no certificate problem stores no exception, so its config is
    // byte-for-byte what it was before exceptions existed.
    assert!(!config.contains("certificate_exception"));
}

/// A certificate the person accepted rides into the stored config, so every later connect of
/// this account carries it and nobody is asked twice (`docs/certificate-exceptions.md`).
#[test]
fn an_accepted_certificate_reaches_the_stored_config() {
    let SetupForm::Detected(DetectedForm::Imap(form)) = imap_form() else {
        panic!("expected IMAP form");
    };

    let config = AccountSubmission::Imap(ImapSubmission {
        email: form.email,
        imap_host: form.imap_host.clone(),
        smtp_host: form.smtp_host,
        caldav_url: form.caldav_url,
        imap_security: form.imap_security,
        smtp_security: form.smtp_security,
        password: "secret".to_owned(),
        accepted_certificate: Some(mailcal_bindings::RejectedCertificate {
            server_name: form.imap_host,
            sha256: "AB:CD".repeat(16),
            subject_common_name: None,
            subject_organisation: None,
            issuer_common_name: None,
            issuer_organisation: None,
            not_before: None,
            not_after: None,
        }),
    })
    .config_toml()
    .expect("valid config");

    assert!(config.contains("certificate_exception"));
}

#[test]
fn setting_up_a_detected_account_by_hand_opens_its_own_type_prefilled() {
    let SetupForm::Detected(detected) = imap_form() else {
        panic!("expected IMAP form");
    };
    let SetupForm::Manual(manual) = edit_manually(&detected) else {
        panic!("expected the manual form");
    };

    assert_eq!(manual.kind, AccountKind::Imap);
    assert_eq!(manual.imap_host, "imap.example.test:143");
    assert_eq!(manual.smtp_host, "smtp.example.test:587");
    assert_eq!(manual.caldav_url, "https://calendar.example.test");

    let SetupForm::Detected(detected) = jmap_form() else {
        panic!("expected JMAP form");
    };
    let SetupForm::Manual(manual) = edit_manually(&detected) else {
        panic!("expected the manual form");
    };

    assert_eq!(manual.kind, AccountKind::Jmap);
    assert_eq!(manual.jmap_server, "https://jmap.example.test");
    assert!(manual.imap_host.is_empty());
}

#[test]
fn a_microsoft_card_set_up_by_hand_stays_a_microsoft_sign_in() {
    let SetupForm::Detected(detected) = recommendation_form(
        SetupRecommendation::Microsoft {
            email: "person@outlook.com".to_owned(),
        },
        String::new(),
    ) else {
        panic!("expected the Microsoft form");
    };
    let SetupForm::Manual(manual) = edit_manually(&detected) else {
        panic!("expected the manual form");
    };

    assert_eq!(manual.kind, AccountKind::Microsoft);
    assert_eq!(manual.email, "person@outlook.com");
}

#[test]
fn the_picker_round_trips_every_account_type() {
    for kind in AccountKind::offered() {
        assert_eq!(AccountKind::from_position(kind.position()), kind);
        assert!(!kind.label().is_empty());
    }
    // A position no row can produce falls back to the first type rather than panicking.
    assert_eq!(AccountKind::from_position(9), AccountKind::Imap);
}

#[test]
fn the_manual_jmap_pre_flight_waits_for_an_address_and_runs_once() {
    let SetupForm::Manual(mut manual) = super::manual_form(String::new(), None) else {
        panic!("expected the manual form");
    };
    manual.kind = AccountKind::Jmap;
    assert!(!manual.probes_jmap_sign_in(), "no address to probe yet");

    manual.email = "alice@example.test".to_owned();
    assert!(manual.probes_jmap_sign_in());

    manual.sign_in = JmapSignIn::Unavailable;
    assert!(!manual.probes_jmap_sign_in(), "answered already");

    manual.kind = AccountKind::Imap;
    manual.sign_in = JmapSignIn::Checking;
    assert!(!manual.probes_jmap_sign_in(), "not a JMAP account");
}

fn jmap_form() -> SetupForm {
    recommendation_form(
        SetupRecommendation::Jmap {
            email: "alice@example.test".to_owned(),
            server_url: "https://jmap.example.test".to_owned(),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    )
}

fn imap_form() -> SetupForm {
    recommendation_form(
        SetupRecommendation::Imap {
            email: "alice@example.test".to_owned(),
            imap_host: "imap.example.test:143".to_owned(),
            smtp_host: Some("smtp.example.test:587".to_owned()),
            imap_security: ConnectionSecurity::StartTls,
            smtp_security: ConnectionSecurity::StartTls,
            incoming: detected("IMAP", 143),
            outgoing: Some(detected("SMTP", 587)),
            caldav_url: Some("https://calendar.example.test".to_owned()),
            is_trusted: true,
            source: "fixture".to_owned(),
        },
        String::new(),
    )
}

fn detected(protocol: &str, port: u16) -> mailcal_bindings::DetectedServerRow {
    mailcal_bindings::DetectedServerRow {
        protocol: protocol.to_owned(),
        hostname: format!("{}.example.test", protocol.to_ascii_lowercase()),
        port,
        security: "STARTTLS".to_owned(),
        username: "alice@example.test".to_owned(),
    }
}

const fn form_kind(form: &SetupForm) -> &'static str {
    match form {
        SetupForm::Detected(DetectedForm::Imap(_)) => "imap",
        SetupForm::Detected(DetectedForm::Jmap(_)) => "jmap",
        SetupForm::Detected(DetectedForm::Microsoft(_)) => "microsoft",
        SetupForm::Detected(DetectedForm::Google(_)) => "google",
        SetupForm::Manual(_) => "manual",
    }
}
