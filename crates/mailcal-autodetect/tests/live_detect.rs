//! Live end-to-end detection against real public providers. Skipped unless
//! `AUTODETECT_LIVE=1`, exactly like `mailcal-account`'s live JMAP test; it needs the
//! network and third-party endpoints, so it never runs in the offline default suite.
//!
//! Run with: `AUTODETECT_LIVE=1 cargo test -p mailcal-autodetect --test live_detect -- --nocapture`

use mailcal_autodetect::{DetectConfig, Detected, detect};

fn enabled() -> bool {
    std::env::var("AUTODETECT_LIVE").as_deref() == Ok("1")
}

#[tokio::test]
async fn gmail_resolves_to_imap_via_the_ispdb() {
    if !enabled() {
        eprintln!("skipping live detect test: AUTODETECT_LIVE unset");
        return;
    }
    let detected = detect("someone@gmail.com", None, &DetectConfig::default())
        .await
        .expect("detection runs");
    match detected {
        Detected::Mail(settings) => {
            assert!(
                settings
                    .incoming
                    .iter()
                    .any(|s| s.hostname.contains("imap")),
                "expected an imap incoming server, got {settings:?}"
            );
        }
        other => panic!("expected mail settings for gmail.com, got {other:?}"),
    }
}

#[tokio::test]
async fn fastmail_without_a_resolver_uses_autoconfig_imap() {
    // Fastmail advertises JMAP only via a `_jmap._tcp` SRV record; its apex
    // `/.well-known/jmap` 302s to a 404. With no host resolver the core can't consult SRV,
    // so it correctly settles on Fastmail's published autoconfig IMAP settings. The
    // SRV→JMAP path itself is proven deterministically by the orchestrator tests
    // (`a_jmap_srv_endpoint_beats_a_lower_priority_mail_config`) and end-to-end by each
    // client's on-device run, which passes its native resolver.
    if !enabled() {
        eprintln!("skipping live detect test: AUTODETECT_LIVE unset");
        return;
    }
    let detected = detect("someone@fastmail.com", None, &DetectConfig::default())
        .await
        .expect("detection runs");
    match detected {
        Detected::Mail(settings) => assert!(
            settings
                .incoming
                .iter()
                .any(|server| server.hostname.contains("fastmail")),
            "expected fastmail imap settings, got {settings:?}"
        ),
        other => {
            panic!("expected imap settings for fastmail.com without a resolver, got {other:?}")
        }
    }
}
