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
fn predefined_entities_and_character_references_resolve_in_text() {
    // The reader hands each `&…;` back on its own, so resolving them is the parser's
    // job: a value split across literal runs and references must come back whole.
    let parsed = parse_with(
        "<username>%EMAILADDRESS%</username>",
        "<username>a&#108;ice&amp;billing@%EMAILDOMAIN%</username>",
    )
    .unwrap();
    assert_eq!(parsed.incoming[0].username, "alice&billing@example.com");
    assert_eq!(parsed.outgoing[0].username, "alice&billing@example.com");
}

#[test]
fn a_custom_entity_does_not_expand() {
    // A DTD-defined entity reference must not expand (no billion-laughs): it is
    // undeclared as far as this parser is concerned, and the document is refused.
    let xml = r#"<?xml version="1.0"?>
<!DOCTYPE clientConfig [<!ENTITY lol "haha">]>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <domain>&lol;</domain>
  </emailProvider>
</clientConfig>"#;
    let err = parse(xml).unwrap_err();
    assert!(matches!(err, ParseError::Xml(_)), "unexpected {err:?}");
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

/// The `<oAuth2>` block as the format writes it: a bare-host issuer beside endpoints and a
/// client id we deliberately never read.
const OAUTH_BLOCK: &str = r"    <oAuth2>
      <issuer>login.example.com</issuer>
      <authURL>https://login.example.com/authorize</authURL>
      <tokenURL>https://login.example.com/token</tokenURL>
      <scope>IMAP SMTP offline_access</scope>
      <clientID>not-ours</clientID>
    </oAuth2>
";

#[test]
fn the_oauth_issuer_is_read_and_given_the_scheme_the_rfc_requires() {
    // The format writes a bare hostname; RFC 8414 defines an issuer identifier as an HTTPS
    // URL, and the whole well-known path is derived from it, so the scheme is added here
    // rather than at four call sites that would each have to remember.
    let parsed = parse(&MINIMAL.replace(
        "  </emailProvider>",
        &format!("{OAUTH_BLOCK}  </emailProvider>"),
    ))
    .expect("valid document");
    assert_eq!(
        parsed.oauth_issuer.as_deref(),
        Some("https://login.example.com")
    );
    // The servers are unaffected: an OAuth block is extra information, not a different config.
    assert_eq!(parsed.incoming.len(), 1);
    assert_eq!(parsed.outgoing.len(), 1);
}

#[test]
fn a_document_with_no_oauth_block_names_no_issuer() {
    assert_eq!(parse(MINIMAL).expect("valid document").oauth_issuer, None);
}

#[test]
fn an_issuer_written_as_a_full_https_url_is_kept() {
    // Some documents write the identifier out in full. Prefixing it again would produce
    // `https://https://…`, which fails discovery in a way that reads like a server fault.
    let block = OAUTH_BLOCK.replace(
        "<issuer>login.example.com</issuer>",
        "<issuer>https://login.example.com/</issuer>",
    );
    let parsed =
        parse(&MINIMAL.replace("  </emailProvider>", &format!("{block}  </emailProvider>")))
            .expect("valid document");
    assert_eq!(
        parsed.oauth_issuer.as_deref(),
        Some("https://login.example.com")
    );
}

#[test]
fn an_issuer_that_is_not_a_hostname_is_dropped_rather_than_half_understood() {
    // An issuer decides which page a person types their password into. A value we cannot
    // read as a host is not a smaller version of that decision, it is no decision, so the
    // route is simply not offered and the password field stays.
    for value in [
        "http://login.example.com",
        "not a host",
        "",
        "ftp://x.example",
    ] {
        let block = OAUTH_BLOCK.replace(
            "<issuer>login.example.com</issuer>",
            &format!("<issuer>{value}</issuer>"),
        );
        let parsed =
            parse(&MINIMAL.replace("  </emailProvider>", &format!("{block}  </emailProvider>")))
                .expect("valid document");
        assert_eq!(
            parsed.oauth_issuer, None,
            "issuer {value:?} must be dropped"
        );
    }
}
