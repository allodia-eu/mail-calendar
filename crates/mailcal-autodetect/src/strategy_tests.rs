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
    run_from(reply, SourceKind::Autoconfig).await
}

/// The same fetch, attributed to `kind`: which source a document came from is what decides
/// whether its issuer is honoured.
async fn run_from(reply: Reply, kind: SourceKind) -> MailFetch {
    let fetcher = FakeFetch::new().on(URL, reply);
    fetch_mail_config(&fetcher, &url::Url::parse(URL).unwrap(), kind, &email()).await
}

/// `VALID` with an `<oAuth2>` block naming an issuer.
fn with_issuer() -> String {
    VALID.replace(
        "</emailProvider>",
        "<oAuth2><issuer>login.example.com</issuer></oAuth2></emailProvider>",
    )
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

#[tokio::test]
async fn a_provider_describing_itself_over_https_may_name_its_issuer() {
    let MailFetch::Found(settings) =
        run_from(Reply::xml(&with_issuer()), SourceKind::Autoconfig).await
    else {
        panic!("expected a found config");
    };
    assert_eq!(
        settings.oauth_issuer.as_deref(),
        Some("https://login.example.com")
    );
}

#[tokio::test]
async fn the_ispdb_may_not_name_an_issuer_for_somebody_else() {
    // The same bytes, from a third party. Not a judgement about how well that database is
    // curated: an issuer decides which page receives someone's password, and a provider
    // naming it for itself is a different trust decision from a directory naming it for them.
    for kind in [SourceKind::Ispdb, SourceKind::MxIspdb] {
        let MailFetch::Found(settings) = run_from(Reply::xml(&with_issuer()), kind).await else {
            panic!("expected a found config");
        };
        assert_eq!(
            settings.oauth_issuer, None,
            "{kind:?} must not name an issuer"
        );
        // The servers it lists are still used: only the issuer is dropped.
        assert_eq!(settings.incoming[0].hostname, "imap.example.com");
    }
}

#[tokio::test]
async fn an_untrusted_hop_names_no_issuer_even_from_the_provider() {
    // The `http://` autoconfig variants some small providers still publish. Those settings
    // are shown to the user for approval before a password is sent; an issuer read off the
    // same document would decide where that password is *typed*, which that approval does
    // not cover.
    let reply = Reply::Ok {
        status: 200,
        body: with_issuer().into_bytes(),
        trusted: false,
        www_authenticate: false,
    };
    let MailFetch::Found(settings) = run_from(reply, SourceKind::Autoconfig).await else {
        panic!("expected a found config");
    };
    assert_eq!(settings.oauth_issuer, None);
    assert!(!settings.is_trusted);
}
