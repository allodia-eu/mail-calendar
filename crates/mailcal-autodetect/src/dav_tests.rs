//! DAV presence-probe tests: how a terminal response is classified (only an HTTPS `401`/`207`
//! counts), the order the candidates are preferred in (the SRV target before the domain's own
//! `.well-known`, the email domain before the provider's), and CardDAV found beside CalDAV.

use std::{sync::Arc, time::Duration};

use crate::{
    DetectConfig,
    dav::{probe, provider_domain},
    mx::MxResolver,
    test_fakes::{FakeFetch, FakeResolver, Reply},
    types::{DetectedDav, Domain},
};

const EMAIL_CALDAV: &str = "https://example.org/.well-known/caldav";
const EMAIL_CARDDAV: &str = "https://example.org/.well-known/carddav";
const PROVIDER_CALDAV: &str = "https://soverin.net/.well-known/caldav";
const SRV_CALDAV: &str = "https://dav.example.net/.well-known/caldav";

fn email_domain() -> Domain {
    Domain::parse("example.org").unwrap()
}

fn soverin() -> Option<Domain> {
    provider_domain("imap.soverin.net")
}

async fn found(fetch: &FakeFetch, provider: Option<&Domain>) -> DetectedDav {
    probe(
        fetch,
        None,
        &email_domain(),
        provider,
        &DetectConfig::default(),
    )
    .await
}

async fn found_with(fetch: &FakeFetch, resolver: FakeResolver) -> DetectedDav {
    let resolver: Arc<dyn MxResolver> = Arc::new(resolver);
    probe(
        fetch,
        Some(&resolver),
        &email_domain(),
        None,
        &DetectConfig::default(),
    )
    .await
}

#[test]
fn the_provider_domain_is_the_registrable_domain_of_its_host() {
    assert_eq!(soverin(), Domain::parse("soverin.net"));
    assert_eq!(
        provider_domain("api.fastmail.com"),
        Domain::parse("fastmail.com")
    );
}

#[tokio::test]
async fn discovers_caldav_on_the_provider_domain_when_the_email_domain_has_none() {
    // The Soverin shape: example.org (the custom domain) has no calendar, but the provider
    // soverin.net (from imap.soverin.net) advertises one with a 401 challenge.
    let fetch = FakeFetch::new()
        .on(EMAIL_CALDAV, Reply::status(404, true))
        .on(PROVIDER_CALDAV, Reply::unauthorized(true));
    let dav = found(&fetch, soverin().as_ref()).await;
    assert_eq!(dav.caldav_url.as_deref(), Some(PROVIDER_CALDAV));
}

#[tokio::test]
async fn the_email_domain_wins_when_both_advertise_caldav() {
    let fetch = FakeFetch::new()
        .on(EMAIL_CALDAV, Reply::unauthorized(true))
        .on(PROVIDER_CALDAV, Reply::unauthorized(true));
    let dav = found(&fetch, soverin().as_ref()).await;
    assert_eq!(dav.caldav_url.as_deref(), Some(EMAIL_CALDAV));
}

#[tokio::test]
async fn a_207_multistatus_counts_as_present() {
    let fetch = FakeFetch::new()
        .on(EMAIL_CALDAV, Reply::status(404, true))
        .on(PROVIDER_CALDAV, Reply::status(207, true));
    let dav = found(&fetch, soverin().as_ref()).await;
    assert_eq!(dav.caldav_url.as_deref(), Some(PROVIDER_CALDAV));
}

#[tokio::test]
async fn nothing_is_discovered_when_every_candidate_404s() {
    let fetch = FakeFetch::new().default_reply(Reply::status(404, true));
    assert!(found(&fetch, soverin().as_ref()).await.is_empty());
}

#[tokio::test]
async fn an_untrusted_401_is_ignored() {
    // A 401 reached over a non-HTTPS hop must not be offered; we'd send credentials there.
    let fetch = FakeFetch::new()
        .on(EMAIL_CALDAV, Reply::status(404, true))
        .on(PROVIDER_CALDAV, Reply::status(401, false));
    assert!(found(&fetch, soverin().as_ref()).await.is_empty());
}

#[tokio::test]
async fn a_redirect_to_a_website_200_is_not_a_false_positive() {
    // A catch-all domain that 301s .well-known/caldav to its homepage lands on a 200, which
    // is not a DAV signal, so nothing is offered.
    let fetch = FakeFetch::new().default_reply(Reply::status(200, true));
    assert!(found(&fetch, soverin().as_ref()).await.is_empty());
}

#[tokio::test]
async fn an_address_book_is_found_beside_the_calendar() {
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on(EMAIL_CALDAV, Reply::unauthorized(true))
        .on(EMAIL_CARDDAV, Reply::status(207, true));
    let dav = found(&fetch, None).await;
    assert_eq!(
        dav,
        DetectedDav {
            caldav_url: Some(EMAIL_CALDAV.to_owned()),
            carddav_url: Some(EMAIL_CARDDAV.to_owned()),
        }
    );
}

#[tokio::test]
async fn an_address_book_alone_is_found() {
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on(EMAIL_CARDDAV, Reply::unauthorized(true));
    let dav = found(&fetch, None).await;
    assert_eq!(dav.caldav_url, None);
    assert_eq!(dav.carddav_url.as_deref(), Some(EMAIL_CARDDAV));
}

#[tokio::test]
async fn an_srv_record_names_a_calendar_on_another_host() {
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on(SRV_CALDAV, Reply::unauthorized(true));
    let resolver = FakeResolver::failing().srv(
        "_caldavs._tcp.example.org",
        vec![(0, 0, 443, "dav.example.net.")],
        false,
    );
    let dav = found_with(&fetch, resolver).await;
    assert_eq!(dav.caldav_url.as_deref(), Some(SRV_CALDAV));
    assert_eq!(dav.carddav_url, None);
}

#[tokio::test]
async fn the_srv_target_is_preferred_over_the_domains_own_well_known() {
    let fetch = FakeFetch::new()
        .on(EMAIL_CALDAV, Reply::unauthorized(true))
        .on(
            "https://dav.example.net:8443/.well-known/caldav",
            Reply::unauthorized(true),
        );
    let resolver = FakeResolver::failing().srv(
        "_caldavs._tcp.example.org",
        vec![(0, 0, 8443, "dav.example.net")],
        false,
    );
    let dav = found_with(&fetch, resolver).await;
    assert_eq!(
        dav.caldav_url.as_deref(),
        Some("https://dav.example.net:8443/.well-known/caldav")
    );
}

#[tokio::test]
async fn an_srv_record_saying_no_service_falls_back_to_the_well_known() {
    let fetch = FakeFetch::new().on(EMAIL_CALDAV, Reply::unauthorized(true));
    let resolver =
        FakeResolver::failing().srv("_caldavs._tcp.example.org", vec![(0, 0, 0, ".")], false);
    let dav = found_with(&fetch, resolver).await;
    assert_eq!(dav.caldav_url.as_deref(), Some(EMAIL_CALDAV));
}

#[tokio::test]
async fn an_srv_target_answering_over_plain_http_is_ignored() {
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on(SRV_CALDAV, Reply::status(401, false));
    let resolver = FakeResolver::failing().srv(
        "_caldavs._tcp.example.org",
        vec![(0, 0, 443, "dav.example.net")],
        false,
    );
    assert!(found_with(&fetch, resolver).await.is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_host_that_never_answers_costs_only_the_probes_budget() {
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on_after(
            EMAIL_CALDAV,
            Reply::unauthorized(true),
            Duration::from_secs(600),
        );
    let started = tokio::time::Instant::now();
    assert!(found(&fetch, None).await.is_empty());
    let config = DetectConfig::default();
    assert!(started.elapsed() <= config.dns_timeout + config.http_timeout);
}
