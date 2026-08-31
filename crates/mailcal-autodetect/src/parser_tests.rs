//! Autoconfig-parser tests. A single hand-written minimal document (ours, not a ported
//! Thunderbird fixture) is mutated per case, and every distinct [`ParseError`] is
//! pinned: a malformed config must fold to a specific, testable failure, never a
//! lenient partial parse.

use super::{ParseError, parse_autoconfig};
use crate::types::{AuthKind, EmailParts, SocketKind};

/// A minimal, valid autoconfig document: the base every mutation test starts from.
const MINIMAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <domain>example.com</domain>
    <incomingServer type="imap">
      <hostname>imap.example.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.com</hostname>
      <port>465</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

fn email() -> EmailParts {
    EmailParts::parse("alice@example.com").unwrap()
}

fn parse(xml: &str) -> Result<super::ParsedServers, ParseError> {
    parse_autoconfig(xml.as_bytes(), &email())
}

/// Parses `MINIMAL` with `find` replaced by `replace`: the mutation helper.
fn parse_with(find: &str, replace: &str) -> Result<super::ParsedServers, ParseError> {
    let mutated = MINIMAL.replace(find, replace);
    assert_ne!(
        mutated, MINIMAL,
        "mutation {find:?} did not change the document"
    );
    parse(&mutated)
}

#[test]
fn parses_a_minimal_document() {
    let parsed = parse(MINIMAL).unwrap();
    assert_eq!(parsed.incoming.len(), 1);
    assert_eq!(parsed.outgoing.len(), 1);

    let imap = &parsed.incoming[0];
    assert_eq!(imap.hostname, "imap.example.com");
    assert_eq!(imap.port, 993);
    assert_eq!(imap.socket, SocketKind::Tls);
    assert_eq!(imap.auth, [AuthKind::PasswordCleartext]);
    assert_eq!(imap.username, "alice@example.com"); // %EMAILADDRESS% substituted

    let smtp = &parsed.outgoing[0];
    assert_eq!(smtp.hostname, "smtp.example.com");
    assert_eq!(smtp.port, 465);
}

#[test]
fn substitutes_all_three_placeholders() {
    let xml = MINIMAL.replace(
        "<username>%EMAILADDRESS%</username>",
        "<username>%EMAILLOCALPART%@%EMAILDOMAIN%</username>",
    );
    let parsed = parse(&xml).unwrap();
    assert_eq!(parsed.incoming[0].username, "alice@example.com");
}

#[test]
fn substitutes_placeholders_in_hostname_and_domain() {
    let xml = MINIMAL
        .replace(
            "<domain>example.com</domain>",
            "<domain>%EMAILDOMAIN%</domain>",
        )
        .replace("imap.example.com", "imap.%EMAILDOMAIN%");
    let parsed = parse(&xml).unwrap();
    assert_eq!(parsed.incoming[0].hostname, "imap.example.com");
}

#[test]
fn missing_client_config_root() {
    assert_eq!(
        parse_with("clientConfig", "wrongRoot").unwrap_err(),
        ParseError::MissingClientConfig
    );
}

#[test]
fn missing_email_provider() {
    let xml = r#"<clientConfig version="1.1"></clientConfig>"#;
    assert_eq!(parse(xml).unwrap_err(), ParseError::MissingEmailProvider);
}

#[test]
fn missing_provider_id() {
    assert_eq!(
        parse_with(r#"<emailProvider id="example.com">"#, "<emailProvider>").unwrap_err(),
        ParseError::MissingProviderId
    );
}

#[test]
fn invalid_provider_id() {
    let err = parse_with(r#"id="example.com""#, r#"id="not a hostname""#).unwrap_err();
    assert!(matches!(err, ParseError::InvalidProviderId(id) if id == "not a hostname"));
}

#[test]
fn no_valid_domain() {
    assert_eq!(
        parse_with(
            "<domain>example.com</domain>",
            "<domain>not a domain</domain>"
        )
        .unwrap_err(),
        ParseError::NoValidDomain
    );
}

#[test]
fn missing_hostname() {
    assert_eq!(
        parse_with("<hostname>imap.example.com</hostname>", "").unwrap_err(),
        ParseError::MissingHostname
    );
}

#[test]
fn invalid_hostname() {
    let err = parse_with("imap.example.com", "imap example com").unwrap_err();
    assert!(matches!(err, ParseError::InvalidHostname(h) if h == "imap example com"));
}

#[test]
fn invalid_port_zero() {
    assert_eq!(
        parse_with("<port>993</port>", "<port>0</port>").unwrap_err(),
        ParseError::InvalidPort
    );
}

#[test]
fn invalid_port_non_numeric() {
    assert_eq!(
        parse_with("<port>993</port>", "<port>imap</port>").unwrap_err(),
        ParseError::InvalidPort
    );
}

#[test]
fn invalid_port_out_of_range() {
    assert_eq!(
        parse_with("<port>993</port>", "<port>70000</port>").unwrap_err(),
        ParseError::InvalidPort
    );
}

#[test]
fn missing_username() {
    assert_eq!(
        parse_with("<username>%EMAILADDRESS%</username>", "").unwrap_err(),
        ParseError::MissingUsername
    );
}

#[test]
fn plaintext_socket_type_is_rejected() {
    let err = parse_with(
        "<socketType>SSL</socketType>",
        "<socketType>plain</socketType>",
    )
    .unwrap_err();
    assert!(matches!(err, ParseError::InvalidSocketType(s) if s == "plain"));
}

#[test]
fn unknown_socket_type_is_rejected() {
    // Even "TLS" is not a valid autoconfig socketType: only "SSL"/"STARTTLS" are.
    let err = parse_with(
        "<socketType>SSL</socketType>",
        "<socketType>TLS</socketType>",
    )
    .unwrap_err();
    assert!(matches!(err, ParseError::InvalidSocketType(s) if s == "TLS"));
}

#[test]
fn starttls_socket_type_is_parsed() {
    let parsed = parse_with(
        "<socketType>SSL</socketType>",
        "<socketType>STARTTLS</socketType>",
    )
    .unwrap();
    assert_eq!(parsed.incoming[0].socket, SocketKind::StartTls);
    assert_eq!(parsed.outgoing[0].socket, SocketKind::StartTls);
}

#[test]
fn no_usable_authentication() {
    assert_eq!(
        parse_with(
            "<authentication>password-cleartext</authentication>",
            "<authentication>NTLM</authentication>",
        )
        .unwrap_err(),
        ParseError::NoUsableAuth
    );
}

#[test]
fn unknown_authentication_is_skipped_when_a_usable_one_remains() {
    let xml = MINIMAL.replace(
        "<authentication>password-cleartext</authentication>",
        "<authentication>NTLM</authentication>\n      <authentication>OAuth2</authentication>",
    );
    let parsed = parse(&xml).unwrap();
    assert_eq!(parsed.incoming[0].auth, [AuthKind::OAuth2]);
}

#[test]
fn multiple_authentications_keep_document_order() {
    let xml = MINIMAL.replace(
        "<authentication>password-cleartext</authentication>",
        "<authentication>OAuth2</authentication>\n      <authentication>password-cleartext</authentication>",
    );
    let parsed = parse(&xml).unwrap();
    assert_eq!(
        parsed.incoming[0].auth,
        [AuthKind::OAuth2, AuthKind::PasswordCleartext]
    );
}

#[test]
fn pop3_incoming_is_skipped_leaving_no_incoming() {
    assert_eq!(
        parse_with(
            r#"<incomingServer type="imap">"#,
            r#"<incomingServer type="pop3">"#
        )
        .unwrap_err(),
        ParseError::NoIncomingServer
    );
}

#[test]
fn missing_outgoing_server() {
    let xml = MINIMAL
        .split("<outgoingServer")
        .next()
        .map(|head| format!("{head}</emailProvider>\n</clientConfig>"))
        .unwrap();
    assert_eq!(parse(&xml).unwrap_err(), ParseError::NoOutgoingServer);
}

#[test]
fn pop3_server_before_imap_is_skipped_and_imap_kept() {
    let pop3 = r#"<incomingServer type="pop3">
      <hostname>pop.example.com</hostname>
      <port>995</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    "#;
    let xml = MINIMAL.replace(
        r#"<incomingServer type="imap">"#,
        &format!("{pop3}<incomingServer type=\"imap\">"),
    );
    let parsed = parse(&xml).unwrap();
    assert_eq!(parsed.incoming.len(), 1);
    assert_eq!(parsed.incoming[0].hostname, "imap.example.com");
}

#[test]
fn multiple_imap_servers_are_kept_in_order() {
    let second = r#"<incomingServer type="imap">
      <hostname>imap2.example.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    "#;
    let xml = MINIMAL.replace(
        r#"<outgoingServer type="smtp">"#,
        &format!("{second}<outgoingServer type=\"smtp\">"),
    );
    let parsed = parse(&xml).unwrap();
    let hosts: Vec<&str> = parsed
        .incoming
        .iter()
        .map(|s| s.hostname.as_str())
        .collect();
    assert_eq!(hosts, ["imap.example.com", "imap2.example.com"]);
}

#[test]
fn top_level_oauth2_block_is_ignored() {
    let xml = MINIMAL.replace(
        "</emailProvider>",
        "</emailProvider>\n  <oAuth2>\n    <issuer>accounts.example.com</issuer>\n    <authURL>https://accounts.example.com/auth</authURL>\n  </oAuth2>",
    );
    // The oAuth2 endpoints are ignored; parsing still succeeds from the servers alone.
    assert!(parse(&xml).is_ok());
}

#[test]
fn unknown_elements_are_skipped() {
    let xml = MINIMAL.replace(
        "<domain>example.com</domain>",
        "<domain>example.com</domain>\n    <displayName>Example</displayName>\n    <documentation url=\"https://example.com\"><descr>How to</descr></documentation>",
    );
    assert!(parse(&xml).is_ok());
}

#[test]
fn a_custom_entity_does_not_expand() {
    // A DTD-defined entity reference must not expand (no billion-laughs); quick-xml
    // rejects the unknown entity, which we surface as a plain XML error.
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE clientConfig [<!ENTITY lol "haha">]>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <domain>&lol;</domain>
  </emailProvider>
</clientConfig>"#;
    let err = parse(xml).unwrap_err();
    assert!(
        matches!(err, ParseError::Xml(_) | ParseError::NoValidDomain),
        "unexpected {err:?}"
    );
}

#[test]
fn truncated_document_errors_without_panicking() {
    let xml = &MINIMAL[..MINIMAL.len() / 2];
    // Either a hard XML error or a "missing field" error, never a panic or a hang.
    assert!(parse(xml).is_err());
}

#[test]
fn malformed_xml_is_reported() {
    // A mismatched end tag inside clientConfig is a hard XML error.
    assert!(matches!(
        parse("<clientConfig><x></y></clientConfig>").unwrap_err(),
        ParseError::Xml(_)
    ));
}
