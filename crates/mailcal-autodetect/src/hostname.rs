//! Hostname validation for detection inputs and parsed autoconfig values.
//!
//! A fresh implementation of the classic RFC 1123 hostname shape (dot-separated
//! labels of ASCII letters/digits/hyphens); deliberately **not** ported from
//! Thunderbird, whose validator descends from MPL-licensed comm-central code.
//! Unicode names are expected to arrive already IDNA/punycode-encoded (the `url`
//! crate does that for typed email domains); a raw Unicode hostname here is
//! invalid, matching Thunderbird's behaviour for autoconfig payloads.

use std::net::{Ipv4Addr, Ipv6Addr};

/// Longest hostname the DNS wire format can carry (RFC 1035 §2.3.4, presentation form).
const MAX_HOSTNAME_LEN: usize = 253;

/// Longest single label (RFC 1035 §2.3.4).
const MAX_LABEL_LEN: usize = 63;

/// Whether `s` is a valid DNS hostname: 1–253 chars of dot-separated labels, each
/// 1–63 ASCII letters/digits/hyphens with no hyphen at either end. Trailing dots
/// and empty labels are rejected.
pub(crate) fn valid_hostname(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_HOSTNAME_LEN && s.split('.').all(valid_label)
}

/// Whether `s` is a valid hostname **or** an IP literal (dotted IPv4, bare IPv6, or
/// bracketed IPv6). Autoconfig files may point a server at an IP literal.
pub(crate) fn valid_host_or_ip(s: &str) -> bool {
    valid_hostname(s) || s.parse::<Ipv4Addr>().is_ok() || is_ipv6_literal(s)
}

/// Whether `s` is an IPv6 address, bare (`::1`) or bracketed (`[::1]`).
fn is_ipv6_literal(s: &str) -> bool {
    let inner = s
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(s);
    inner.parse::<Ipv6Addr>().is_ok()
}

/// One dot-separated hostname label: 1–63 ASCII alphanumerics/hyphens, not
/// hyphen-edged.
fn valid_label(label: &str) -> bool {
    (1..=MAX_LABEL_LEN).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

#[cfg(test)]
mod tests {
    use super::{valid_host_or_ip, valid_hostname};

    #[test]
    fn accepts_ordinary_hostnames() {
        for host in [
            "example.com",
            "imap.example.com",
            "a.b.c.d.example.co.uk",
            "localhost",
            "test.local",
            "xn--mnchen-3ya.de",
            "host-with-hyphens.example.com",
            "1.example.com",
        ] {
            assert!(valid_hostname(host), "{host} should be valid");
        }
    }

    #[test]
    fn rejects_malformed_hostnames() {
        for host in [
            "",
            ".",
            ".example.com",
            "example.com.",
            "exa mple.com",
            "under_score.example.com",
            "-leading.example.com",
            "trailing-.example.com",
            "bücher.de",
            "host..double-dot.com",
            "host:993",
        ] {
            assert!(!valid_hostname(host), "{host} should be invalid");
        }
    }

    #[test]
    fn rejects_over_length_names_and_labels() {
        let long_label = format!("{}.example.com", "a".repeat(64));
        assert!(!valid_hostname(&long_label));
        assert!(valid_hostname(&format!("{}.example.com", "a".repeat(63))));

        let long_name = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(63),
        ]
        .join(".");
        assert!(!valid_hostname(&long_name), "{} chars", long_name.len());
    }

    #[test]
    fn rejects_ipv6_literals() {
        for ip in ["::1", "[::1]", "2001:db8::2:1"] {
            assert!(!valid_hostname(ip), "{ip} is an IP, not a hostname");
        }
    }

    #[test]
    fn host_or_ip_also_accepts_ip_literals() {
        for value in [
            "imap.example.com",
            "192.168.0.1",
            "::1",
            "[::1]",
            "2001:db8::2:1",
        ] {
            assert!(valid_host_or_ip(value), "{value} should be a host-or-ip");
        }
        // Note: "999.999.999.999" is a valid *hostname* (all-numeric DNS labels are
        // legal) even though it is not a valid IPv4 address, so it is intentionally not
        // in this reject list.
        for value in ["", "bad host.com", "[not-an-ip]", "under_score.com"] {
            assert!(!valid_host_or_ip(value), "{value} should be rejected");
        }
    }
}
