//! Looking again for a server that stopped answering, and which answers a stored password may
//! follow.

use std::sync::Arc;

use super::{discover_with, may_follow, site_of};
use crate::{
    DetectConfig,
    mx::MxResolver,
    test_fakes::{FakeFetch, FakeResolver, Reply},
};

const EMAIL: &str = "alice@example.com";
const STORED: &str = "https://d100.caldav.example.com/.well-known/caldav";

#[test]
fn a_new_host_of_the_same_provider_is_followed() {
    assert!(may_follow(
        "https://d200.caldav.example.com/.well-known/caldav",
        STORED,
        EMAIL
    ));
    assert!(
        may_follow("https://dav.example.com/", STORED, "alice@example.org"),
        "under the registrable domain of the server it replaces"
    );
    assert!(
        may_follow(
            "https://calendar.example.org/",
            "https://dav.example.net/",
            "alice@example.org"
        ),
        "under the registrable domain of the address"
    );
    assert!(may_follow(STORED, STORED, EMAIL), "the same server");
}

#[test]
fn a_server_anywhere_else_is_not_followed() {
    assert!(!may_follow(
        "https://dav.elsewhere.example.net/",
        STORED,
        EMAIL
    ));
    assert!(
        !may_follow("http://d200.caldav.example.com/", STORED, EMAIL),
        "never to plain HTTP"
    );
    assert!(!may_follow("not a url", STORED, EMAIL));
    assert!(
        !may_follow(
            "https://127.0.0.2/dav",
            "https://127.0.0.1/dav",
            "alice@test.local"
        ),
        "an address the suffix list cannot place is followed only when it is the same host"
    );
    assert!(may_follow(
        "https://127.0.0.1/dav/new",
        "https://127.0.0.1/dav",
        "alice@test.local"
    ));
}

/// The SRV target moved: discovery asked again finds the new host.
#[tokio::test]
async fn discovery_asked_again_finds_where_the_srv_record_points_now() {
    let moved = "https://d200.caldav.example.com/.well-known/caldav";
    let fetch = FakeFetch::new()
        .default_reply(Reply::status(404, true))
        .on(moved, Reply::unauthorized(true));
    let resolver: Arc<dyn MxResolver> = Arc::new(FakeResolver::failing().srv(
        "_caldavs._tcp.example.com",
        vec![(0, 1, 443, "d200.caldav.example.com.")],
        false,
    ));

    let found = discover_with(
        &fetch,
        EMAIL,
        Some("d100.caldav.example.com"),
        Some(&resolver),
        &DetectConfig::default(),
    )
    .await
    .expect("a valid address");

    assert_eq!(found.caldav_url.as_deref(), Some(moved));
    assert!(may_follow(moved, STORED, EMAIL));
}

#[test]
fn a_site_is_the_registrable_domain_a_person_recognises() {
    assert_eq!(
        site_of("https://p01-caldav.icloud.com/123/calendars/").as_deref(),
        Some("icloud.com")
    );
    assert_eq!(
        site_of("https://caldav.example.co.uk/dav/").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(site_of("https://localhost/dav/"), None);
    assert_eq!(site_of("not a url"), None);
}
