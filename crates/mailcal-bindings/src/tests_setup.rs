//! The setup surface's ports and connection security: the numbers a manual form offers, and
//! that a server typed by hand reaches the port and the security it was given. Its own file
//! because `tests.rs` is at the 500-line limit.

use super::*;

#[test]
fn the_standard_port_is_the_one_the_core_assumes_for_that_security() {
    // The form shows these and the core assumes them when a host carries no port. They are
    // asserted together because the whole point of the export is that they cannot diverge.
    assert_eq!(
        standard_port(MailServerKind::Imap, ConnectionSecurity::ImplicitTls),
        993
    );
    assert_eq!(
        standard_port(MailServerKind::Imap, ConnectionSecurity::StartTls),
        143
    );
    assert_eq!(
        standard_port(MailServerKind::Smtp, ConnectionSecurity::ImplicitTls),
        465
    );
    assert_eq!(
        standard_port(MailServerKind::Smtp, ConnectionSecurity::StartTls),
        587
    );

    for security in [
        ConnectionSecurity::ImplicitTls,
        ConnectionSecurity::StartTls,
    ] {
        let config = account_config_toml(AccountSetup {
            imap_host: "imap.example.net".to_owned(),
            username: "someone@example.net".to_owned(),
            password: "secret".to_owned(),
            smtp_host: Some("smtp.example.net".to_owned()),
            caldav_base_url: None,
            imap_security: Some(security),
            smtp_security: Some(security),
            accepted_certificate: None,
        })
        .expect("valid account config");
        assert!(
            config.contains(&format!(
                "imap.example.net:{}",
                standard_port(MailServerKind::Imap, security)
            )),
            "a host typed without a port dials the port the form offered for {security:?}"
        );
        assert!(
            config.contains(&format!(
                "smtp.example.net:{}",
                standard_port(MailServerKind::Smtp, security)
            )),
            "and the same holds for submission"
        );
    }
}

#[test]
fn a_hand_typed_starttls_server_keeps_the_port_it_was_given() {
    // The manual form's whole purpose: a server autodetection cannot find, on a port that is
    // nobody's standard. Proton Mail Bridge is the case this was written for.
    let config = account_config_toml(AccountSetup {
        imap_host: "127.0.0.1:1143".to_owned(),
        username: "someone@example.net".to_owned(),
        password: "secret".to_owned(),
        smtp_host: Some("127.0.0.1:1025".to_owned()),
        caldav_base_url: None,
        imap_security: Some(ConnectionSecurity::StartTls),
        smtp_security: Some(ConnectionSecurity::StartTls),
        accepted_certificate: None,
    })
    .expect("valid account config");

    assert!(config.contains("127.0.0.1:1143"), "the typed port stands");
    assert!(config.contains("127.0.0.1:1025"));
    assert_eq!(
        config.matches("starttls").count(),
        2,
        "both servers are recorded as STARTTLS, so neither is dialled as implicit TLS"
    );
}
