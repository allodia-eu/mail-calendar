//! Tests for the parts of the setup decision that decide something without a network.
//!
//! The decision itself needs a mail server to answer and an issuer to publish metadata, and
//! is exercised end to end against the local Stalwart harness rather than mocked: a mock that
//! answers whatever it is asked proves the code calls itself. What is worth pinning offline
//! is the arithmetic around it, where a wrong answer is silent: which port a probe dials, and
//! which issuers it is willing to ask.

use super::*;

fn query(imap_host: &str, email: &str) -> ImapAuthQuery {
    ImapAuthQuery {
        imap_host: imap_host.to_owned(),
        imap_security: ConnectionSecurity::ImplicitTls,
        email: email.to_owned(),
        autoconfig_issuer: None,
    }
}

#[test]
fn a_bare_host_gets_the_standard_port_for_its_security() {
    // The probe must reach the same server the connect will. A probe that dialled 993 on a
    // STARTTLS account would answer a question about a listener the account never uses, and
    // the two disagree about exactly the thing being asked: which mechanisms are offered.
    assert_eq!(
        dial_addr("imap.example.net", ConnectionSecurity::ImplicitTls),
        "imap.example.net:993"
    );
    assert_eq!(
        dial_addr("imap.example.net", ConnectionSecurity::StartTls),
        "imap.example.net:143"
    );
    // An explicit port is preserved, including a non-standard one.
    assert_eq!(
        dial_addr("imap.example.net:12993", ConnectionSecurity::ImplicitTls),
        "imap.example.net:12993"
    );
}

#[test]
fn the_host_is_read_off_a_server_field_that_may_carry_a_port() {
    assert_eq!(host_of("imap.example.net"), "imap.example.net");
    assert_eq!(host_of("imap.example.net:993"), "imap.example.net");
    // Not a port: left alone rather than truncated into a host that resolves to nothing.
    assert_eq!(host_of("imap.example.net:imaps"), "imap.example.net:imaps");
}

#[test]
fn the_autoconfig_issuer_is_asked_first() {
    // The provider naming its own authorization server is better evidence than a guess at a
    // well-known path, and asking it first means the guesses usually never run.
    let mut query = query("imap.example.net", "alice@example.com");
    query.autoconfig_issuer = Some("https://login.example.net".to_owned());
    let candidates = issuer_candidates(&query, "imap.example.net");
    assert_eq!(candidates.first().unwrap(), "https://login.example.net");
}

#[test]
fn the_email_domain_is_asked_before_the_servers_own() {
    // A self-hosted server answers as `mail.example.com` while its mailboxes are
    // `@example.com`, and the metadata lives on the domain people actually have accounts at.
    let candidates = issuer_candidates(
        &query("mail.example.com", "alice@example.com"),
        "mail.example.com",
    );
    assert_eq!(
        candidates,
        ["https://example.com", "https://mail.example.com",]
    );
}

#[test]
fn a_hosted_provider_is_reached_through_its_own_registrable_domain() {
    // The custom-domain case: the address says nothing about who runs the mailbox, and the
    // server host is the only thing that does.
    let candidates = issuer_candidates(
        &query("imap.provider.example", "alice@customdomain.example"),
        "imap.provider.example",
    );
    assert_eq!(
        candidates,
        [
            "https://customdomain.example",
            "https://provider.example",
            "https://imap.provider.example",
        ]
    );
}

#[test]
fn no_candidate_is_asked_twice() {
    // When the email domain and the server's registrable domain coincide, which is the
    // ordinary self-hosted case, the same metadata request must not be made twice.
    let candidates = issuer_candidates(&query("example.com", "alice@example.com"), "example.com");
    assert_eq!(candidates, ["https://example.com"]);
}

#[test]
fn an_address_with_no_domain_still_yields_the_servers_own_candidates() {
    // A half-typed address reaches this path when the user corrected the server by hand.
    // Producing no candidates at all would report `RegistrationNeeded` for a server that may
    // well publish metadata.
    let candidates = issuer_candidates(&query("imap.example.net", "alice"), "imap.example.net");
    assert_eq!(
        candidates,
        ["https://example.net", "https://imap.example.net"]
    );
}

#[test]
fn a_registrable_domain_is_the_last_two_labels() {
    assert_eq!(registrable_domain("imap.mail.example.com"), "example.com");
    assert_eq!(registrable_domain("example.com"), "example.com");
    // A single label has no shorter form to derive; returned unchanged rather than emptied.
    assert_eq!(registrable_domain("localhost"), "localhost");
}

/// A host that accepts connections and then says nothing, returning its port. A mail host with
/// nothing behind its 443 behaves this way, as does a server that stalls after the handshake, and
/// a request to either has no end of its own.
async fn silent_host() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    port
}

#[tokio::test]
async fn a_mail_server_that_never_answers_cannot_hold_the_setup_screen() {
    let query = query(
        &format!("127.0.0.1:{}", silent_host().await),
        "alice@example.invalid",
    );

    let budget = Duration::from_millis(300);
    let offer = tokio::time::timeout(Duration::from_secs(3), probe(&query, budget))
        .await
        .expect("the capability probe outlived its own budget");
    assert!(offer.is_none());
}

#[tokio::test]
async fn an_issuer_that_never_answers_cannot_hold_the_setup_screen() {
    let port = silent_host().await;
    let query = ImapAuthQuery {
        autoconfig_issuer: Some(format!("https://127.0.0.1:{port}")),
        ..query("imap.example.invalid", "alice@example.invalid")
    };

    let budget = Duration::from_millis(300);
    let found = tokio::time::timeout(Duration::from_secs(3), imap_issuer_within(&query, budget))
        .await
        .expect("the issuer search outlived its own budget");
    assert!(found.is_none());
}
