//! A mail server's address as a person writes it and as an account stores it: a host, and the
//! port after it when one is given. The one split and join every form and the core use, so a form
//! never shows one port while the core dials another (`docs/account-autodetect.md` rule 11).
//!
//! An IPv6 literal is written in brackets when a port follows it (`[::1]:993`), as a URL writes
//! it. Bare (`::1`), it names no port: its last group would otherwise read as one.

/// `address` as its host and its port, the port `""` when it names none. Only digits after the
/// one colon outside brackets are a port: `imap.example.org:993`, `[::1]:993`. A bare IPv6
/// literal and anything else that is not a port stay part of the host, and a port with nothing
/// before it (`:993`, a cleared host beside the port its form kept) has an empty host.
#[must_use]
pub fn split_server_address(address: &str) -> (&str, &str) {
    let address = address.trim();
    if address.starts_with('[')
        && let Some(close) = address.find(']')
    {
        let (host, rest) = address.split_at(close + 1);
        return match rest.strip_prefix(':') {
            _ if rest.is_empty() => (host, ""),
            Some(port) if is_port(port) => (host, port),
            _ => (address, ""),
        };
    }
    match address.split_once(':') {
        Some((host, port)) if is_port(port) => (host, port),
        _ => (address, ""),
    }
}

/// The address of `host` at `port`: the host alone when no port is given or there is no host,
/// and an IPv6 literal put in brackets before its port.
#[must_use]
pub fn join_server_address(host: &str, port: &str) -> String {
    let (host, port) = (host.trim(), port.trim());
    if port.is_empty() || host.is_empty() {
        host.to_owned()
    } else if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// The host `address` names, as its certificate names it: without its port, and an IPv6 literal
/// without its brackets.
#[must_use]
pub fn server_address_host(address: &str) -> &str {
    server_name(split_server_address(address).0)
}

/// The name a server's certificate is checked against: its host without the brackets an IPv6
/// literal is written in.
pub(crate) fn server_name(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host)
}

/// Digits, or nothing: a colon with no port after it names none.
fn is_port(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "server_address_tests.rs"]
mod tests;
