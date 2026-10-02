//! The manual form's port and connection-security rule, as arithmetic. The GTK client's copy of
//! the suite every client carries for [`super::setup_server_field`].
//!
//! Plain `#[test]`s: nothing here touches GTK.

use mailcal_bindings::{ConnectionSecurity, MailServerKind};

use super::setup_server_field::{ServerField, split_host};

#[test]
fn a_fresh_field_offers_the_standard_secure_port() {
    assert_eq!(ServerField::new(MailServerKind::Imap).port(), "993");
    assert_eq!(ServerField::new(MailServerKind::Smtp).port(), "465");
    assert_eq!(
        ServerField::new(MailServerKind::Imap).security(),
        ConnectionSecurity::ImplicitTls
    );
}

#[test]
fn the_port_follows_the_picker_while_it_is_still_ours() {
    let mut imap = ServerField::new(MailServerKind::Imap);
    imap.choose_security(ConnectionSecurity::StartTls);
    assert_eq!(imap.port(), "143");
    imap.choose_security(ConnectionSecurity::ImplicitTls);
    assert_eq!(imap.port(), "993");

    let mut smtp = ServerField::new(MailServerKind::Smtp);
    smtp.choose_security(ConnectionSecurity::StartTls);
    assert_eq!(smtp.port(), "587");
}

#[test]
fn a_port_typed_by_hand_is_never_overwritten_by_the_picker() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.type_port("1143");

    field.choose_security(ConnectionSecurity::StartTls);
    assert_eq!(field.port(), "1143");
    field.choose_security(ConnectionSecurity::ImplicitTls);
    assert_eq!(field.port(), "1143", "the manual form's whole purpose");
    assert!(!field.follows_security());
}

#[test]
fn clearing_the_port_hands_it_back_to_the_picker() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.type_port("1143");
    field.type_port("   ");

    assert!(field.follows_security());
    assert_eq!(field.port(), "993");
    field.choose_security(ConnectionSecurity::StartTls);
    assert_eq!(field.port(), "143");
}

#[test]
fn the_dial_address_carries_the_host_and_the_port_together() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.choose_security(ConnectionSecurity::StartTls);
    assert_eq!(field.dial("imap.example.net"), "imap.example.net:143");

    field.type_port("1143");
    assert_eq!(field.dial("127.0.0.1"), "127.0.0.1:1143");
    assert_eq!(field.dial("  127.0.0.1  "), "127.0.0.1:1143");
}

#[test]
fn a_port_already_typed_into_the_host_field_wins() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.type_port("1143");
    assert_eq!(field.dial("127.0.0.1:1025"), "127.0.0.1:1025");
}

#[test]
fn an_empty_host_stays_empty_so_the_connect_gate_still_refuses_it() {
    assert_eq!(ServerField::new(MailServerKind::Imap).dial("   "), "");
}

#[test]
fn a_detected_route_brings_its_own_port_and_stops_following_the_picker() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.adopt_detected("imap.example.net:1993", ConnectionSecurity::StartTls);

    assert_eq!(field.port(), "1993");
    assert!(!field.follows_security());
    assert_eq!(field.security(), ConnectionSecurity::StartTls);
}

#[test]
fn a_detected_route_without_a_port_shows_the_standard_one_for_what_was_detected() {
    let mut field = ServerField::new(MailServerKind::Imap);
    field.adopt_detected("imap.example.net", ConnectionSecurity::StartTls);

    assert_eq!(field.port(), "143");
    assert!(field.follows_security());
}

#[test]
fn only_an_all_digit_tail_is_a_port() {
    assert_eq!(split_host("imap.example.net"), ("imap.example.net", ""));
    assert_eq!(
        split_host("imap.example.net:993"),
        ("imap.example.net", "993")
    );
    assert_eq!(split_host("host:"), ("host:", ""));
    assert_eq!(split_host("host:abc"), ("host:abc", ""));
    // Mirrors the core, which splits the same way; a bare IPv6 literal is not a server either
    // side accepts.
    assert_eq!(split_host("::1"), (":", "1"));
}
