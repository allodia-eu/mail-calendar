//! Orchestration tests: priority ordering and the deadline under `tokio` paused time,
//! plus the MX-fallback gating, over the fake fetcher/resolver.

use std::{sync::Arc, time::Duration};

use super::orchestrate;
use crate::{
    DetectConfig,
    fetch::Fetch,
    mx::MxResolver,
    test_fakes::{FakeFetch, FakeResolver, Reply},
    types::{Detected, EmailParts, SourceKind},
    urls,
};

const CONFIG: &str = r#"<clientConfig version="1.1"><emailProvider id="example.com">
<domain>example.com</domain>
<incomingServer type="imap"><hostname>imap.example.com</hostname><port>993</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></incomingServer>
<outgoingServer type="smtp"><hostname>smtp.example.com</hostname><port>465</port>
<socketType>SSL</socketType><username>%EMAILADDRESS%</username>
<authentication>password-cleartext</authentication></outgoingServer>
</emailProvider></clientConfig>"#;

/// The autoconfig endpoint for a domain hosting mail at `google.com` via MX.
const MX_AUTOCONFIG: &str = "https://autoconfig.google.com/mail/config-v1.1.xml";

/// The CalDAV well-known on the provider domain of `CONFIG`'s incoming host
/// (`imap.example.com` → `example.com`); what the follow-on probe reaches.
const PROVIDER_CALDAV: &str = "https://example.com/.well-known/caldav";

fn email() -> EmailParts {
    EmailParts::parse("user@company.example").unwrap()
}

fn jmap_url() -> String {
    urls::jmap_well_known(&email().domain, None).to_string()
}

fn autoconfig_url() -> String {
    urls::autoconfig_urls(&email().domain)[0].to_string()
}

fn ispdb_url() -> String {
    urls::ispdb_url(&email().domain).to_string()
}

async fn run(fetcher: FakeFetch, resolver: Option<FakeResolver>, config: DetectConfig) -> Detected {
    let fetcher: Arc<dyn Fetch> = Arc::new(fetcher);
    let resolver = resolver.map(|resolver| Arc::new(resolver) as Arc<dyn MxResolver>);
    orchestrate(fetcher, email(), resolver, config).await
}

#[tokio::test(start_paused = true)]
async fn jmap_wins_over_available_mail_config() {
    // Autoconfig and ISPDB would both find IMAP settings, but the JMAP probe is priority 0.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::xml(CONFIG))
        .on(&jmap_url(), Reply::json(r#"{"capabilities":{}}"#, true));
    assert!(matches!(
        run(fetcher, None, DetectConfig::default()).await,
        Detected::Jmap(_)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_slower_higher_priority_strategy_still_wins() {
    // Autoconfig (priority 1) is slow; ISPDB (priority 2) is instant. Autoconfig must
    // still win, because a lower-priority success waits for the higher one to finish.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on_after(
            &autoconfig_url(),
            Reply::xml(CONFIG),
            Duration::from_secs(3),
        )
        .on(&ispdb_url(), Reply::xml(CONFIG));
    let Detected::Mail(mail) = run(fetcher, None, DetectConfig::default()).await else {
        panic!("expected mail settings");
    };
    assert_eq!(mail.source.kind, SourceKind::Autoconfig);
}

#[tokio::test(start_paused = true)]
async fn every_transport_failure_reports_offline() {
    let fetcher = FakeFetch::new().default_reply(Reply::Net);
    assert_eq!(
        run(fetcher, None, DetectConfig::default()).await,
        Detected::Nothing {
            network_error: true
        }
    );
}

#[tokio::test(start_paused = true)]
async fn clean_misses_report_nothing_not_offline() {
    let fetcher = FakeFetch::new().default_reply(Reply::Miss);
    assert_eq!(
        run(fetcher, None, DetectConfig::default()).await,
        Detected::Nothing {
            network_error: false
        }
    );
}

#[tokio::test(start_paused = true)]
async fn the_overall_deadline_stops_a_hanging_run() {
    // The JMAP probe would succeed, but only long after the deadline.
    let fetcher = FakeFetch::new().default_reply(Reply::Miss).on_after(
        &jmap_url(),
        Reply::json(r#"{"capabilities":{}}"#, true),
        Duration::from_secs(100),
    );
    let config = DetectConfig {
        overall_deadline: Duration::from_secs(10),
        ..DetectConfig::default()
    };
    assert_eq!(
        run(fetcher, None, config).await,
        Detected::Nothing {
            network_error: false
        }
    );
}

#[tokio::test]
async fn the_mx_fallback_runs_when_a_resolver_is_present() {
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(MX_AUTOCONFIG, Reply::xml(CONFIG));
    let resolver = FakeResolver::with(vec![(10, "mx.google.com.")], true);
    let Detected::Mail(mail) = run(fetcher, Some(resolver), DetectConfig::default()).await else {
        panic!("expected mail settings from the MX fallback");
    };
    assert_eq!(mail.source.kind, SourceKind::MxAutoconfig);
}

#[tokio::test]
async fn a_found_mail_config_carries_a_discovered_caldav_endpoint() {
    // Autoconfig finds IMAP (imap.example.com); the follow-on RFC 6764 probe then finds a
    // CalDAV service on the provider domain (example.com) and it rides along on the Mail
    // result: the calendar endpoint mail settings alone never carry.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(&autoconfig_url(), Reply::xml(CONFIG))
        .on(PROVIDER_CALDAV, Reply::unauthorized(true));
    let Detected::Mail(mail) = run(fetcher, None, DetectConfig::default()).await else {
        panic!("expected mail settings");
    };
    assert_eq!(mail.caldav_url.as_deref(), Some(PROVIDER_CALDAV));
}

#[tokio::test(start_paused = true)]
async fn the_mx_fallback_is_skipped_without_a_resolver() {
    // The MX-derived config is available, but with no resolver the MX strategy never runs.
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(MX_AUTOCONFIG, Reply::xml(CONFIG));
    assert_eq!(
        run(fetcher, None, DetectConfig::default()).await,
        Detected::Nothing {
            network_error: false
        }
    );
}

#[tokio::test]
async fn a_jmap_srv_endpoint_beats_a_lower_priority_mail_config() {
    // The Fastmail shape: the apex advertises no JMAP, but `_jmap._tcp` does, while ISPDB
    // would return IMAP settings. Because the JMAP probe (priority 0) now consults SRV, it
    // wins over the ISPDB IMAP result: the domain routes to JMAP, not IMAP.
    let srv_target = "https://api.company.example/.well-known/jmap";
    let fetcher = FakeFetch::new()
        .default_reply(Reply::Miss)
        .on(&ispdb_url(), Reply::xml(CONFIG))
        .on(srv_target, Reply::unauthorized(true));
    let resolver = FakeResolver::with(vec![], false).srv(
        "_jmap._tcp.company.example",
        vec![(0, 1, 443, "api.company.example.")],
        true,
    );
    let Detected::Jmap(jmap) = run(fetcher, Some(resolver), DetectConfig::default()).await else {
        panic!("expected a JMAP route from the SRV endpoint");
    };
    assert_eq!(jmap.base_url, "https://api.company.example");
    assert_eq!(jmap.source.kind, SourceKind::JmapSrv);
}

#[tokio::test]
async fn the_imap_srv_strategy_resolves_when_higher_strategies_miss() {
    // No JMAP, no autoconfig, no ISPDB, but the domain publishes `_imaps`/`_submissions`
    // SRV records, so the IMAP/SMTP SRV strategy (priority 3) produces the config.
    let fetcher = FakeFetch::new().default_reply(Reply::Miss);
    let resolver = FakeResolver::with(vec![], false)
        .srv(
            "_imaps._tcp.company.example",
            vec![(0, 1, 993, "imap.company.example.")],
            true,
        )
        .srv(
            "_submissions._tcp.company.example",
            vec![(0, 1, 465, "smtp.company.example.")],
            true,
        );
    let Detected::Mail(mail) = run(fetcher, Some(resolver), DetectConfig::default()).await else {
        panic!("expected mail settings from the IMAP SRV strategy");
    };
    assert_eq!(mail.source.kind, SourceKind::ImapSrv);
    assert_eq!(mail.incoming[0].hostname, "imap.company.example");
    assert_eq!(mail.outgoing[0].hostname, "smtp.company.example");
}
