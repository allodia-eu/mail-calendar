//! The Autodiscover strategy over the fake fetcher and resolver: which answers name Microsoft 365,
//! and that nothing else does.

use std::sync::Arc;

use super::run;
use crate::{
    DetectConfig,
    fetch::Fetch,
    mx::MxResolver,
    strategy::StrategyOutcome,
    test_fakes::{FakeFetch, FakeResolver, Reply},
    types::{EmailParts, SourceKind},
};

const PROBE: &str = "http://autodiscover.company.example/Autodiscover/Autodiscover.xml";
const MICROSOFT: &str = "https://autodiscover-s.outlook.com/Autodiscover/Autodiscover.xml";
const SRV: &str = "_autodiscover._tcp.company.example";

async fn outcome(fetcher: FakeFetch, resolver: Option<FakeResolver>) -> StrategyOutcome {
    let fetcher: Arc<dyn Fetch> = Arc::new(fetcher);
    let resolver = resolver.map(|resolver| Arc::new(resolver) as Arc<dyn MxResolver>);
    let email = EmailParts::parse("user@Company.Example").unwrap();
    run(fetcher, email, resolver, DetectConfig::default()).await
}

fn landed(url: &str) -> Reply {
    Reply::Landed {
        final_url: url.to_owned(),
        status: 401,
    }
}

#[tokio::test]
async fn a_redirect_to_microsoft_names_microsoft_365() {
    let fetcher = FakeFetch::new().on(PROBE, landed(MICROSOFT));
    let StrategyOutcome::Microsoft365(found) = outcome(fetcher, None).await else {
        panic!("a redirect to Exchange Online is Microsoft 365");
    };
    assert_eq!(found.source.kind, SourceKind::Autodiscover);
    // The URL of the probe, built from the normalised domain, never the address.
    assert_eq!(found.source.url, PROBE);
}

#[tokio::test]
async fn an_srv_record_naming_microsoft_names_microsoft_365() {
    let resolver =
        FakeResolver::failing().srv(SRV, vec![(0, 0, 443, "autodiscover.outlook.com.")], false);
    let StrategyOutcome::Microsoft365(found) = outcome(FakeFetch::new(), Some(resolver)).await
    else {
        panic!("an SRV record naming Exchange Online is Microsoft 365");
    };
    assert_eq!(found.source.url, SRV);
}

#[tokio::test]
async fn a_self_hosted_exchange_is_nothing_found() {
    // Self-hosted Exchange needs an Exchange protocol this product does not speak yet.
    let fetcher = FakeFetch::new().on(
        PROBE,
        landed("https://mail.company.example/Autodiscover/Autodiscover.xml"),
    );
    let resolver =
        FakeResolver::failing().srv(SRV, vec![(0, 0, 443, "mail.company.example")], false);
    assert!(matches!(
        outcome(fetcher, Some(resolver)).await,
        StrategyOutcome::Nothing
    ));
}

#[tokio::test]
async fn a_lookalike_host_is_not_microsoft() {
    for host in [
        "https://autodiscover.outlook.com.attacker.example/Autodiscover/Autodiscover.xml",
        "https://outlook.com/Autodiscover/Autodiscover.xml",
        "https://autodiscover.partner.outlook.cn/Autodiscover/Autodiscover.xml",
    ] {
        let fetcher = FakeFetch::new().on(PROBE, landed(host));
        assert!(
            matches!(outcome(fetcher, None).await, StrategyOutcome::Nothing),
            "{host} was taken for Microsoft 365"
        );
    }
}

#[tokio::test]
async fn an_unreachable_probe_is_a_network_error() {
    let fetcher = FakeFetch::new().on(PROBE, Reply::Net);
    assert!(matches!(
        outcome(fetcher, None).await,
        StrategyOutcome::NetworkError
    ));
}
