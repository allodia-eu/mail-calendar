//! RFC 6186/8314 mail-SRV strategy tests, over the fake resolver.

use std::sync::Arc;

use super::run;
use crate::{
    DetectConfig,
    mx::MxResolver,
    strategy::StrategyOutcome,
    test_fakes::FakeResolver,
    types::{AuthKind, EmailParts, SocketKind, SourceKind},
};

const IMAPS: &str = "_imaps._tcp.company.example";
const IMAP: &str = "_imap._tcp.company.example";
const SUBMISSIONS: &str = "_submissions._tcp.company.example";
const SUBMISSION: &str = "_submission._tcp.company.example";

fn email() -> EmailParts {
    EmailParts::parse("user@company.example").unwrap()
}

async fn run_srv(resolver: FakeResolver) -> StrategyOutcome {
    let resolver: Arc<dyn MxResolver> = Arc::new(resolver);
    run(email(), resolver, DetectConfig::default()).await
}

#[tokio::test]
async fn finds_imap_and_smtp_from_srv_records() {
    let resolver = FakeResolver::with(vec![], false)
        .srv(IMAPS, vec![(0, 1, 993, "imap.company.example.")], true)
        .srv(
            SUBMISSIONS,
            vec![(0, 1, 465, "smtp.company.example.")],
            true,
        );
    let StrategyOutcome::Mail(settings) = run_srv(resolver).await else {
        panic!("expected mail settings from SRV");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.company.example");
    assert_eq!(settings.incoming[0].port, 993);
    assert_eq!(settings.incoming[0].socket, SocketKind::Tls);
    assert_eq!(settings.incoming[0].auth, vec![AuthKind::PasswordCleartext]);
    // RFC 6186 carries no username: the full typed address is the default.
    assert_eq!(settings.incoming[0].username, "user@company.example");
    assert_eq!(settings.outgoing[0].hostname, "smtp.company.example");
    assert_eq!(settings.outgoing[0].port, 465);
    assert_eq!(settings.source.kind, SourceKind::ImapSrv);
    assert!(settings.is_trusted, "implicit-TLS servers ⇒ trusted");
}

#[tokio::test]
async fn imap_only_leaves_submission_unconfigured() {
    // A domain publishing only `_imaps` and neither submission label: mail-read works, send
    // stays unconfigured rather than blocking.
    let resolver = FakeResolver::with(vec![], false).srv(
        IMAPS,
        vec![(0, 1, 993, "imap.company.example.")],
        true,
    );
    let StrategyOutcome::Mail(settings) = run_srv(resolver).await else {
        panic!("expected mail settings from SRV");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.company.example");
    assert!(
        settings.outgoing.is_empty(),
        "no submission record ⇒ empty outgoing"
    );
    assert!(settings.is_trusted, "implicit-TLS IMAP ⇒ trusted");
}

#[tokio::test]
async fn falls_back_to_starttls_labels_when_implicit_absent() {
    // A domain publishing only the STARTTLS labels (`_imap` 143, `_submission` 587); the
    // engine now speaks STARTTLS, so both are configured, marked STARTTLS.
    let resolver = FakeResolver::with(vec![], false)
        .srv(IMAP, vec![(0, 1, 143, "imap.company.example.")], true)
        .srv(SUBMISSION, vec![(0, 1, 587, "smtp.company.example.")], true);
    let StrategyOutcome::Mail(settings) = run_srv(resolver).await else {
        panic!("expected mail settings from STARTTLS SRV");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.company.example");
    assert_eq!(settings.incoming[0].port, 143);
    assert_eq!(settings.incoming[0].socket, SocketKind::StartTls);
    assert_eq!(settings.outgoing[0].hostname, "smtp.company.example");
    assert_eq!(settings.outgoing[0].port, 587);
    assert_eq!(settings.outgoing[0].socket, SocketKind::StartTls);
    assert!(settings.is_trusted, "STARTTLS over validated TLS ⇒ trusted");
}

#[tokio::test]
async fn prefers_implicit_tls_when_both_labels_are_published() {
    // When a domain publishes both, implicit TLS wins (RFC 8314's preference): the STARTTLS
    // record is never consulted for that transport.
    let resolver = FakeResolver::with(vec![], false)
        .srv(IMAPS, vec![(0, 1, 993, "imaps.company.example.")], true)
        .srv(IMAP, vec![(0, 1, 143, "imap.company.example.")], true)
        .srv(
            SUBMISSIONS,
            vec![(0, 1, 465, "smtps.company.example.")],
            true,
        )
        .srv(SUBMISSION, vec![(0, 1, 587, "smtp.company.example.")], true);
    let StrategyOutcome::Mail(settings) = run_srv(resolver).await else {
        panic!("expected mail settings from SRV");
    };
    assert_eq!(settings.incoming[0].hostname, "imaps.company.example");
    assert_eq!(settings.incoming[0].socket, SocketKind::Tls);
    assert_eq!(settings.outgoing[0].hostname, "smtps.company.example");
    assert_eq!(settings.outgoing[0].socket, SocketKind::Tls);
}

#[tokio::test]
async fn an_srv_mail_config_is_trusted_over_tls_without_dnssec() {
    // No DNSSEC (AD=false), and the target may even be cross-domain (a custom domain hosted
    // by a provider); trusted, because the engine validates the implicit-TLS cert on connect
    // and the resolved host is pinned into the stored config.
    let resolver = FakeResolver::with(vec![], false).srv(
        IMAPS,
        vec![(0, 1, 993, "imap.mailhost.example.")],
        false,
    );
    let StrategyOutcome::Mail(settings) = run_srv(resolver).await else {
        panic!("expected mail settings");
    };
    assert_eq!(settings.incoming[0].hostname, "imap.mailhost.example");
    assert!(
        settings.is_trusted,
        "validated TLS ⇒ trusted without DNSSEC"
    );
}

#[tokio::test]
async fn no_imap_srv_is_nothing() {
    // A submission record with no `_imaps` yields no incoming server: no mail config.
    let resolver = FakeResolver::with(vec![], false).srv(
        SUBMISSIONS,
        vec![(0, 1, 465, "smtp.company.example.")],
        true,
    );
    assert!(matches!(run_srv(resolver).await, StrategyOutcome::Nothing));
}

#[tokio::test]
async fn a_no_service_sentinel_target_is_skipped() {
    // The RFC 2782 "." target: a provider explicitly not offering the service.
    let resolver = FakeResolver::with(vec![], false).srv(IMAPS, vec![(0, 0, 0, ".")], true);
    assert!(matches!(run_srv(resolver).await, StrategyOutcome::Nothing));
}
