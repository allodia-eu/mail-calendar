//! Tests for the shared fetch-then-parse helper.

use super::{MailFetch, fetch_mail_config};
use crate::{
    test_fakes::{FakeFetch, Reply},
    types::{EmailParts, SourceKind},
};

const VALID: &str = r#"<clientConfig version="1.1"><emailProvider id="example.com">
<domain>example.com</domain>
<incomingServer type="imap"><hostname>imap.example.com</hostname><port>993</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></incomingServer>
<outgoingServer type="smtp"><hostname>smtp.example.com</hostname><port>465</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></outgoingServer>
</emailProvider></clientConfig>"#;

const URL: &str = "https://autoconfig.example.com/mail/config-v1.1.xml";

fn email() -> EmailParts {
    EmailParts::parse("alice@example.com").unwrap()
}

async fn run(reply: Reply) -> MailFetch {
    let fetcher = FakeFetch::new().on(URL, reply);
    fetch_mail_config(
        &fetcher,
        &url::Url::parse(URL).unwrap(),
        SourceKind::Autoconfig,
        &email(),
    )
    .await
}

#[tokio::test]
async fn a_valid_config_is_found_with_its_source_and_trust() {
    let MailFetch::Found(settings) = run(Reply::xml(VALID)).await else {
        panic!("expected a found config");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.example.com");
    assert!(settings.is_trusted);
    assert_eq!(settings.source.kind, SourceKind::Autoconfig);
    assert_eq!(settings.source.url, URL);
}

#[tokio::test]
async fn an_unparseable_body_is_a_miss() {
    assert!(matches!(run(Reply::xml("<nope/>")).await, MailFetch::Miss));
}

#[tokio::test]
async fn a_non_success_status_is_a_miss() {
    let reply = Reply::Ok {
        status: 404,
        body: Vec::new(),
        trusted: true,
        www_authenticate: false,
    };
    assert!(matches!(run(reply).await, MailFetch::Miss));
}

#[tokio::test]
async fn a_transport_failure_is_a_network_error() {
    assert!(matches!(run(Reply::Net).await, MailFetch::NetworkError));
}
