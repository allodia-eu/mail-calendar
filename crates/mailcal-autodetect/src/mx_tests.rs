//! MX-fallback tests: the pure derivations, then the strategy over fakes.

use std::sync::Arc;

use super::{base_domain, most_preferred, run, sub_domain, usable_srv_targets};
use crate::{
    DetectConfig,
    fetch::Fetch,
    mx::{MxRecord, MxResolver, SrvRecord, SrvResolution},
    strategy::StrategyOutcome,
    test_fakes::{FakeFetch, FakeResolver, Reply},
    types::EmailParts,
};

const CONFIG: &str = r#"<clientConfig version="1.1"><emailProvider id="google.com">
<domain>google.com</domain>
<incomingServer type="imap"><hostname>imap.gmail.com</hostname><port>993</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></incomingServer>
<outgoingServer type="smtp"><hostname>smtp.gmail.com</hostname><port>465</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></outgoingServer>
</emailProvider></clientConfig>"#;

/// The provider autoconfig URL for the MX-derived base domain `google.com`.
const GOOGLE_AUTOCONFIG: &str = "https://autoconfig.google.com/mail/config-v1.1.xml";

fn record(preference: u16, exchange: &str) -> MxRecord {
    MxRecord {
        preference,
        exchange: exchange.to_owned(),
    }
}

#[test]
fn most_preferred_takes_the_lowest_preference_normalized() {
    let records = [
        record(20, "backup.google.com."),
        record(10, "MX.Google.COM."),
        record(30, "other.google.com."),
    ];
    assert_eq!(most_preferred(&records).unwrap(), "mx.google.com");
}

#[test]
fn most_preferred_breaks_ties_by_first() {
    let records = [record(10, "a.example.com"), record(10, "b.example.com")];
    assert_eq!(most_preferred(&records).unwrap(), "a.example.com");
}

#[test]
fn base_domain_uses_the_public_suffix_list() {
    assert_eq!(
        base_domain("aspmx.l.google.com").unwrap().as_str(),
        "google.com"
    );
    assert_eq!(base_domain("mx.foo.co.uk").unwrap().as_str(), "foo.co.uk");
    assert_eq!(
        base_domain("eu-smtp.mail.protection.outlook.com")
            .unwrap()
            .as_str(),
        "outlook.com"
    );
    // A bare public suffix has no registrable domain.
    assert!(base_domain("co.uk").is_none());
}

#[test]
fn sub_domain_strips_the_first_label() {
    assert_eq!(
        sub_domain("mx.something.emailprovider.example")
            .unwrap()
            .as_str(),
        "something.emailprovider.example"
    );
    assert!(sub_domain("localhost").is_none());
}

#[test]
fn usable_srv_targets_sorts_by_priority_and_drops_sentinels() {
    let srv = |priority, port, target: &str| SrvRecord {
        priority,
        weight: 1,
        port,
        target: target.to_owned(),
    };
    let resolution = SrvResolution {
        records: vec![
            srv(20, 993, "b.example."),
            srv(0, 0, "zero-port.example."), // dropped: a 0 port is unusable
            srv(10, 993, "a.example."),
            srv(0, 443, "."), // dropped: the RFC 2782 "service not offered" sentinel
        ],
        authentic_data: true,
    };
    let targets: Vec<String> = usable_srv_targets(&resolution)
        .into_iter()
        .map(|record| record.target)
        .collect();
    // Priority-ordered, sentinels dropped, and the DNS trailing dot trimmed.
    assert_eq!(targets, ["a.example", "b.example"]);
}

fn email() -> EmailParts {
    EmailParts::parse("user@company.example").unwrap()
}

async fn run_mx(fetcher: FakeFetch, resolver: FakeResolver) -> StrategyOutcome {
    let fetcher: Arc<dyn Fetch> = Arc::new(fetcher);
    let resolver: Arc<dyn MxResolver> = Arc::new(resolver);
    run(fetcher, email(), resolver, DetectConfig::default()).await
}

#[tokio::test]
async fn resolves_via_the_mx_derived_provider_autoconfig() {
    let fetcher = FakeFetch::new().on(GOOGLE_AUTOCONFIG, Reply::xml(CONFIG));
    let resolver = FakeResolver::with(vec![(10, "mx.google.com.")], true);
    let StrategyOutcome::Mail(settings) = run_mx(fetcher, resolver).await else {
        panic!("expected mail settings");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.gmail.com");
    assert!(settings.is_trusted, "https autoconfig fetch ⇒ trusted");
}

#[tokio::test]
async fn an_mx_result_is_trusted_over_https_without_dnssec() {
    // A Google-Workspace-hosted custom domain: MX → google.com → autoconfig over HTTPS. The
    // servers are TLS-validated at connect, so it's trusted even without DNSSEC (AD=false);
    // unified with the SRV strategies.
    let fetcher = FakeFetch::new().on(GOOGLE_AUTOCONFIG, Reply::xml(CONFIG));
    let resolver = FakeResolver::with(vec![(10, "mx.google.com.")], false);
    let StrategyOutcome::Mail(settings) = run_mx(fetcher, resolver).await else {
        panic!("expected mail settings");
    };
    assert!(settings.is_trusted, "https fetch ⇒ trusted without DNSSEC");
}

#[tokio::test]
async fn no_mx_records_is_nothing() {
    let outcome = run_mx(FakeFetch::new(), FakeResolver::with(vec![], true)).await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn a_failed_lookup_is_a_network_error() {
    let outcome = run_mx(FakeFetch::new(), FakeResolver::failing()).await;
    assert!(
        matches!(outcome, StrategyOutcome::NetworkError),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn an_mx_on_the_same_base_domain_exits_early() {
    // company.example's MX is mail.company.example → base company.example == the email
    // domain, so there is nothing new to look up.
    let resolver = FakeResolver::with(vec![(10, "mail.company.example.")], true);
    let outcome = run_mx(FakeFetch::new().default_reply(Reply::xml(CONFIG)), resolver).await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}
