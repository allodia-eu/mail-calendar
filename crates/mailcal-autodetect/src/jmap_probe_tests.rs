//! JMAP-probe classification tests, over the fake fetcher (apex) and resolver (SRV).

use std::sync::Arc;

use super::run;
use crate::{
    DetectConfig,
    mx::MxResolver,
    strategy::StrategyOutcome,
    test_fakes::{FakeFetch, FakeResolver, Reply},
    types::{EmailParts, SourceKind},
};

fn email() -> EmailParts {
    EmailParts::parse("alice@fastmail.example").unwrap()
}

/// The `_jmap._tcp` owner name for the test email's domain.
const SRV_NAME: &str = "_jmap._tcp.fastmail.example";
/// The well-known on the SRV target the tests point at.
const SRV_TARGET_URL: &str = "https://api.fastmail.example/.well-known/jmap";

async fn probe(fetcher: FakeFetch, config: &DetectConfig) -> StrategyOutcome {
    run(&fetcher, &email(), None, config).await
}

async fn probe_with(
    fetcher: FakeFetch,
    resolver: FakeResolver,
    config: &DetectConfig,
) -> StrategyOutcome {
    let resolver: Arc<dyn MxResolver> = Arc::new(resolver);
    run(&fetcher, &email(), Some(&resolver), config).await
}

fn default(reply: Reply) -> FakeFetch {
    FakeFetch::new().default_reply(reply)
}

#[tokio::test]
async fn a_json_session_with_capabilities_is_jmap() {
    let outcome = probe(
        default(Reply::json(r#"{"capabilities":{},"accounts":{}}"#, true)),
        &DetectConfig::default(),
    )
    .await;
    let StrategyOutcome::Jmap(jmap) = outcome else {
        panic!("expected jmap, got {outcome:?}");
    };
    assert_eq!(jmap.base_url, "https://fastmail.example");
    assert!(jmap.is_trusted);
}

#[tokio::test]
async fn a_401_with_www_authenticate_is_jmap() {
    let outcome = probe(default(Reply::unauthorized(true)), &DetectConfig::default()).await;
    assert!(
        matches!(outcome, StrategyOutcome::Jmap(_)),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn a_bare_401_is_not_jmap() {
    let outcome = probe(
        default(Reply::unauthorized(false)),
        &DetectConfig::default(),
    )
    .await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn a_2xx_that_is_not_a_json_object_is_not_jmap() {
    for body in ["<html>hi</html>", r#"["not","an","object"]"#, "null"] {
        let outcome = probe(default(Reply::json(body, true)), &DetectConfig::default()).await;
        assert!(
            matches!(outcome, StrategyOutcome::Nothing),
            "{body:?} → {outcome:?}"
        );
    }
}

#[tokio::test]
async fn a_json_object_without_capabilities_is_not_jmap() {
    let outcome = probe(
        default(Reply::json(r#"{"accounts":{}}"#, true)),
        &DetectConfig::default(),
    )
    .await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn a_transport_failure_is_a_network_error() {
    let outcome = probe(default(Reply::Net), &DetectConfig::default()).await;
    assert!(
        matches!(outcome, StrategyOutcome::NetworkError),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn an_untrusted_http_positive_is_rejected_without_the_override() {
    // A capabilities session reached over a non-HTTPS hop proves nothing about an HTTPS
    // JMAP endpoint, so it is not accepted.
    let outcome = probe(
        default(Reply::json(r#"{"capabilities":{}}"#, false)),
        &DetectConfig::default(),
    )
    .await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn the_harness_override_accepts_http_and_uses_the_override_base() {
    let config = DetectConfig {
        well_known_base_override: Some(url::Url::parse("http://127.0.0.1:18080").unwrap()),
        ..DetectConfig::default()
    };
    let outcome = probe(
        default(Reply::json(r#"{"capabilities":{}}"#, false)),
        &config,
    )
    .await;
    let StrategyOutcome::Jmap(jmap) = outcome else {
        panic!("expected jmap, got {outcome:?}");
    };
    assert_eq!(jmap.base_url, "http://127.0.0.1:18080");
}

#[tokio::test]
async fn an_srv_target_is_probed_when_the_apex_misses() {
    // The Fastmail shape: the apex advertises no JMAP, but `_jmap._tcp` points at a host
    // that does. The stored base is that target's origin, and a DNSSEC-authentic SRV is
    // trusted.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(SRV_TARGET_URL, Reply::unauthorized(true));
    let resolver = FakeResolver::with(vec![], false).srv(
        SRV_NAME,
        vec![(0, 1, 443, "api.fastmail.example.")],
        true,
    );
    let StrategyOutcome::Jmap(jmap) = probe_with(fetcher, resolver, &DetectConfig::default()).await
    else {
        panic!("expected jmap via SRV");
    };
    assert_eq!(jmap.base_url, "https://api.fastmail.example");
    assert_eq!(jmap.source.kind, SourceKind::JmapSrv);
    assert!(jmap.is_trusted, "DNSSEC-authenticated SRV ⇒ trusted");
}

#[tokio::test]
async fn an_srv_endpoint_is_trusted_over_validated_tls_without_dnssec() {
    // Fastmail runs no DNSSEC, but the endpoint is reached over CA-validated TLS (and pinned
    // into the stored config), so it's trusted: no scary warning for a normal setup.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(SRV_TARGET_URL, Reply::unauthorized(true));
    let resolver = FakeResolver::with(vec![], false).srv(
        SRV_NAME,
        vec![(0, 1, 443, "api.fastmail.example.")],
        false,
    );
    let StrategyOutcome::Jmap(jmap) = probe_with(fetcher, resolver, &DetectConfig::default()).await
    else {
        panic!("expected jmap via SRV");
    };
    assert!(jmap.is_trusted, "validated TLS ⇒ trusted without DNSSEC");
}

#[tokio::test]
async fn a_cross_domain_provider_srv_target_is_trusted() {
    // The RFC 8620 / jmap.io shape: a domain's `_jmap._tcp` points at the provider's host in
    // a *different* registrable domain (example.fm → api.fastmail.com). That's the norm, so
    // it's trusted over validated TLS; email domain `fastmail.example`, target `mailhost`.
    let cross = "https://jmap.mailhost.example/.well-known/jmap";
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(cross, Reply::unauthorized(true));
    let resolver = FakeResolver::with(vec![], false).srv(
        SRV_NAME,
        vec![(0, 1, 443, "jmap.mailhost.example.")],
        false,
    );
    let StrategyOutcome::Jmap(jmap) = probe_with(fetcher, resolver, &DetectConfig::default()).await
    else {
        panic!("expected jmap via cross-domain SRV");
    };
    assert!(
        jmap.is_trusted,
        "cross-domain provider SRV ⇒ trusted over TLS"
    );
}

#[tokio::test]
async fn an_apex_hit_skips_the_srv_lookup() {
    // The apex speaks JMAP, so the SRV record (which points elsewhere) is never consulted.
    let fetcher = FakeFetch::new().on(
        "https://fastmail.example/.well-known/jmap",
        Reply::json(r#"{"capabilities":{}}"#, true),
    );
    let resolver = FakeResolver::with(vec![], false).srv(
        SRV_NAME,
        vec![(0, 1, 443, "api.fastmail.example.")],
        true,
    );
    let StrategyOutcome::Jmap(jmap) = probe_with(fetcher, resolver, &DetectConfig::default()).await
    else {
        panic!("expected jmap");
    };
    assert_eq!(jmap.base_url, "https://fastmail.example");
    assert_eq!(jmap.source.kind, SourceKind::JmapWellKnown);
}

#[tokio::test]
async fn an_srv_lookup_failure_is_a_clean_nothing() {
    let fetcher = FakeFetch::new().default_reply(Reply::Miss);
    let resolver = FakeResolver::with(vec![], false).srv_failing(SRV_NAME);
    let outcome = probe_with(fetcher, resolver, &DetectConfig::default()).await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn an_srv_target_that_does_not_speak_jmap_is_nothing() {
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(SRV_TARGET_URL, Reply::status(404, true));
    let resolver = FakeResolver::with(vec![], false).srv(
        SRV_NAME,
        vec![(0, 1, 443, "api.fastmail.example.")],
        true,
    );
    let outcome = probe_with(fetcher, resolver, &DetectConfig::default()).await;
    assert!(
        matches!(outcome, StrategyOutcome::Nothing),
        "got {outcome:?}"
    );
}
