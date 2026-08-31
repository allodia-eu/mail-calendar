//! CalDAV presence-probe tests: how a terminal response is classified (only an HTTPS
//! `401`/`207` counts), and the two-candidate probe: the account's email domain and the
//! provider's registrable domain, with the email-domain hit winning a tie.

use std::time::Duration;

use crate::{
    caldav::probe,
    test_fakes::{FakeFetch, Reply},
    types::{
        AuthKind, DetectedMailSettings, DetectedServer, EmailParts, SocketKind, Source, SourceKind,
    },
};

const BUDGET: Duration = Duration::from_secs(4);

/// Mail settings whose preferred incoming server is `imap_host` (its registrable domain is
/// the provider domain the probe derives).
fn settings(imap_host: &str) -> DetectedMailSettings {
    DetectedMailSettings {
        incoming: vec![DetectedServer {
            hostname: imap_host.to_owned(),
            port: 993,
            socket: SocketKind::Tls,
            auth: vec![AuthKind::PasswordCleartext],
            username: "info@example.org".to_owned(),
        }],
        outgoing: Vec::new(),
        is_trusted: true,
        source: Source {
            kind: SourceKind::MxAutoconfig,
            url: String::new(),
        },
        caldav_url: None,
    }
}

fn email() -> EmailParts {
    EmailParts::parse("info@example.org").unwrap()
}

const EMAIL_WELL_KNOWN: &str = "https://example.org/.well-known/caldav";
const PROVIDER_WELL_KNOWN: &str = "https://soverin.net/.well-known/caldav";

#[tokio::test]
async fn discovers_caldav_on_the_provider_domain_when_the_email_domain_has_none() {
    // The Soverin shape: example.org (the custom domain) has no calendar, but the provider
    // soverin.net (from imap.soverin.net) advertises one with a 401 challenge.
    let fetch = FakeFetch::new()
        .on(EMAIL_WELL_KNOWN, Reply::status(404, true))
        .on(PROVIDER_WELL_KNOWN, Reply::unauthorized(true));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found.as_deref(), Some(PROVIDER_WELL_KNOWN));
}

#[tokio::test]
async fn the_email_domain_wins_when_both_advertise_caldav() {
    let fetch = FakeFetch::new()
        .on(EMAIL_WELL_KNOWN, Reply::unauthorized(true))
        .on(PROVIDER_WELL_KNOWN, Reply::unauthorized(true));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found.as_deref(), Some(EMAIL_WELL_KNOWN));
}

#[tokio::test]
async fn a_207_multistatus_counts_as_present() {
    let fetch = FakeFetch::new()
        .on(EMAIL_WELL_KNOWN, Reply::status(404, true))
        .on(PROVIDER_WELL_KNOWN, Reply::status(207, true));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found.as_deref(), Some(PROVIDER_WELL_KNOWN));
}

#[tokio::test]
async fn nothing_is_discovered_when_both_domains_404() {
    let fetch = FakeFetch::new().default_reply(Reply::status(404, true));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found, None);
}

#[tokio::test]
async fn an_untrusted_401_is_ignored() {
    // A 401 reached over a non-HTTPS hop must not be offered; we'd send credentials there.
    let fetch = FakeFetch::new()
        .on(EMAIL_WELL_KNOWN, Reply::status(404, true))
        .on(PROVIDER_WELL_KNOWN, Reply::status(401, false));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found, None);
}

#[tokio::test]
async fn a_redirect_to_a_website_200_is_not_a_false_positive() {
    // A catch-all domain that 301s .well-known/caldav to its homepage lands on a 200;
    // which is not a CalDAV signal, so no calendar is offered.
    let fetch = FakeFetch::new().default_reply(Reply::status(200, true));
    let found = probe(&fetch, &email(), &settings("imap.soverin.net"), BUDGET).await;
    assert_eq!(found, None);
}

#[tokio::test]
async fn the_email_domain_is_probed_only_once_when_the_provider_domain_matches() {
    // When mail is hosted on the email domain itself (mail.example.org → example.org), the
    // provider candidate equals the email domain and isn't probed twice; the single hit
    // is returned.
    let fetch = FakeFetch::new().on(EMAIL_WELL_KNOWN, Reply::unauthorized(true));
    let found = probe(&fetch, &email(), &settings("mail.example.org"), BUDGET).await;
    assert_eq!(found.as_deref(), Some(EMAIL_WELL_KNOWN));
}
