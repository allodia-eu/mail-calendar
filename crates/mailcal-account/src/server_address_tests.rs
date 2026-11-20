//! The split and join every server field uses: a host and its port, an IPv6 literal in brackets
//! before a port and bare without one.

use super::{join_server_address, server_address_host, server_name, split_server_address};

#[test]
fn a_host_and_its_port_split_at_the_one_colon() {
    assert_eq!(
        split_server_address("imap.example.org"),
        ("imap.example.org", "")
    );
    assert_eq!(
        split_server_address(" imap.example.org:993 "),
        ("imap.example.org", "993")
    );
    assert_eq!(
        split_server_address("127.0.0.1:12993"),
        ("127.0.0.1", "12993")
    );
    assert_eq!(
        split_server_address("host:"),
        ("host", ""),
        "no port after it"
    );
    assert_eq!(split_server_address("host:abc"), ("host:abc", ""));
}

#[test]
fn a_port_with_nothing_before_it_has_no_host() {
    assert_eq!(split_server_address(":993"), ("", "993"));
    assert_eq!(split_server_address(":"), ("", ""));
    assert_eq!(split_server_address(""), ("", ""));
}

#[test]
fn an_ipv6_literal_is_bracketed_before_its_port_and_bare_without_one() {
    assert_eq!(split_server_address("[::1]:993"), ("[::1]", "993"));
    assert_eq!(split_server_address("[::1]"), ("[::1]", ""));
    assert_eq!(split_server_address("[::1]:"), ("[::1]", ""));
    assert_eq!(
        split_server_address("::1"),
        ("::1", ""),
        "its last group is no port"
    );
    assert_eq!(split_server_address("2001:db8::25"), ("2001:db8::25", ""));
    assert_eq!(split_server_address("[::1]x"), ("[::1]x", ""));
}

#[test]
fn a_host_and_a_port_join_with_an_ipv6_literal_in_brackets() {
    assert_eq!(
        join_server_address("imap.example.org", "993"),
        "imap.example.org:993"
    );
    assert_eq!(
        join_server_address(" imap.example.org ", " "),
        "imap.example.org"
    );
    assert_eq!(join_server_address("::1", "993"), "[::1]:993");
    assert_eq!(join_server_address("[::1]", "993"), "[::1]:993");
    assert_eq!(join_server_address("::1", ""), "::1");
    assert_eq!(join_server_address("", "993"), "", "no host is no address");
}

#[test]
fn what_is_joined_splits_back_into_the_same_host_and_port() {
    for (host, port) in [
        ("imap.example.org", "993"),
        ("[::1]", "1143"),
        ("192.0.2.7", "465"),
    ] {
        assert_eq!(
            split_server_address(&join_server_address(host, port)),
            (host, port)
        );
    }
}

#[test]
fn a_certificate_names_an_ipv6_literal_without_its_brackets() {
    assert_eq!(server_name("[::1]"), "::1");
    assert_eq!(server_name("::1"), "::1");
    assert_eq!(server_name("imap.example.org"), "imap.example.org");
}

#[test]
fn the_host_an_address_names_is_the_one_its_certificate_names() {
    assert_eq!(
        server_address_host("imap.example.org:993"),
        "imap.example.org"
    );
    assert_eq!(server_address_host("[::1]:993"), "::1");
    assert_eq!(server_address_host("::1"), "::1");
}
