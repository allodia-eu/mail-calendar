//! The profile's endpoint matching (§3.4) and host rule (§3.6), against the shapes deployed
//! servers publish.

use super::*;

#[test]
fn an_http_entry_covers_its_own_path_and_below_at_a_segment_boundary() {
    // §3.4's own examples.
    assert!(resource_covers(
        "https://caldav.example.com",
        "https://caldav.example.com/.well-known/caldav"
    ));
    assert!(resource_covers(
        "https://example.com/dav",
        "https://example.com/dav/calendars/"
    ));
    assert!(!resource_covers(
        "https://example.com/dav",
        "https://example.com/davenport"
    ));
    assert!(!resource_covers(
        "https://api.example.com",
        "https://api.example.com.attacker.net/jmap/session"
    ));
    assert!(!resource_covers(
        "https://api.example.com",
        "https://api.example.com:8443/jmap/session"
    ));
    assert!(!resource_covers(
        "https://api.example.com",
        "https://api.example.com@attacker.net/"
    ));
    assert!(!resource_covers(
        "https://caldav.example.com",
        "http://caldav.example.com/"
    ));
}

#[test]
fn a_default_port_matches_an_explicit_one_and_hosts_compare_without_case() {
    assert!(resource_covers(
        "https://CalDAV.Example.com:443",
        "https://caldav.example.com/dav"
    ));
}

#[test]
fn a_bare_host_entry_names_a_non_http_endpoint_only() {
    assert!(resource_covers("imap.example.com", "IMAP.example.com"));
    assert!(!resource_covers("imap.example.com", "smtp.example.com"));
    assert!(!resource_covers(
        "imap.example.com",
        "https://imap.example.com/"
    ));
}

#[test]
fn the_longest_matching_entry_wins() {
    let entries = [
        "https://example.com".to_owned(),
        "https://example.com/dav".to_owned(),
    ];
    assert_eq!(
        covering_entry(&entries, "https://example.com/dav/cal/").as_deref(),
        Some("https://example.com/dav")
    );
    assert_eq!(
        covering_entry(&entries, "https://example.com/jmap").as_deref(),
        Some("https://example.com")
    );
    assert_eq!(covering_entry(&entries, "https://other.example/"), None);
}

#[test]
fn without_a_list_a_token_reaches_the_issuers_registrable_domain_only() {
    // Fastmail: the issuer is api.fastmail.com and its DAV hosts share fastmail.com.
    let none: [String; 0] = [];
    assert!(token_may_reach(
        "https://api.fastmail.com",
        &none,
        "caldav.fastmail.com"
    ));
    assert!(token_may_reach(
        "https://api.fastmail.com",
        &none,
        "d277161.carddav.fastmail.com"
    ));
    assert!(!token_may_reach(
        "https://api.fastmail.com",
        &none,
        "caldav.example.org"
    ));
    // The public suffix list, not "the last two labels": co.uk is a suffix, not a domain.
    assert!(!token_may_reach(
        "https://login.provider.co.uk",
        &none,
        "dav.other.co.uk"
    ));
    assert!(token_may_reach(
        "https://login.provider.co.uk",
        &none,
        "dav.provider.co.uk"
    ));
}

#[test]
fn a_list_replaces_the_domain_rule() {
    let listed = [
        "https://dav.example.com".to_owned(),
        "imap.example.net".to_owned(),
    ];
    assert!(token_may_reach(
        "https://auth.example.com",
        &listed,
        "dav.example.com"
    ));
    assert!(token_may_reach(
        "https://auth.example.com",
        &listed,
        "IMAP.example.net"
    ));
    // On the issuer's own domain, but not listed: the list is the whole answer.
    assert!(!token_may_reach(
        "https://auth.example.com",
        &listed,
        "www.example.com"
    ));
}

#[test]
fn a_loopback_issuer_reaches_its_own_host() {
    // The harness: an issuer on `localhost` has no registrable domain; the host itself stands in.
    let none: [String; 0] = [];
    assert!(token_may_reach(
        "http://localhost:28081",
        &none,
        "localhost"
    ));
    assert!(!token_may_reach(
        "http://localhost:28081",
        &none,
        "127.0.0.1"
    ));
}
