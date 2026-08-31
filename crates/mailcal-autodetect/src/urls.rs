//! URL construction for every lookup strategy; pure functions, pinned by tests.
//!
//! Privacy rule (binding, see `docs/account-autodetect.md`): the **email address never
//! appears in any URL**. The provider's autoconfig endpoints and the ISPDB receive only
//! the domain; Thunderbird's optional `?emailaddress=` query parameter is deliberately
//! not implemented.

use url::Url;

use crate::types::Domain;

/// The ISPDB base; Thunderbird's public database of provider configurations.
const ISPDB_BASE: &str = "https://autoconfig.thunderbird.net/v1.1/";

/// The four Mozilla-autoconfig candidate URLs for the email's own domain, in priority
/// order: both paths over HTTPS first, then the same paths over plain HTTP. The HTTP
/// variants exist because many small providers still publish configs that way
/// (Thunderbird parity); anything fetched over them is marked untrusted and requires
/// explicit user approval.
pub fn autoconfig_urls(domain: &Domain) -> Vec<Url> {
    ["https", "http"]
        .iter()
        .flat_map(|scheme| {
            [
                parse(&format!(
                    "{scheme}://autoconfig.{domain}/mail/config-v1.1.xml"
                )),
                parse(&format!(
                    "{scheme}://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"
                )),
            ]
        })
        .collect()
}

/// The ISPDB lookup URL for `domain` (the domain travels as a path segment; never the
/// address).
pub fn ispdb_url(domain: &Domain) -> Url {
    parse(&format!("{ISPDB_BASE}{domain}"))
}

/// The two URLs tried for an **MX-derived** domain, in order: the provider's own
/// autoconfig endpoint (HTTPS only; after an MX lookup Thunderbird checks no plain-HTTP
/// variant, and neither do we), then the ISPDB.
pub fn post_mx_urls(domain: &Domain) -> Vec<Url> {
    vec![
        parse(&format!("https://autoconfig.{domain}/mail/config-v1.1.xml")),
        ispdb_url(domain),
    ]
}

/// The RFC 6764 CalDAV bootstrap URL for `domain`: `https://{domain}/.well-known/caldav`.
/// A present service redirects this (`301`/`302`) to its context path or answers `401`;
/// an absent one `404`s. HTTPS only: a discovered calendar endpoint we may send
/// credentials to must come from a tamper-resistant hop. Only the domain travels, never
/// the address.
pub fn caldav_well_known(domain: &Domain) -> Url {
    parse(&format!("https://{domain}/.well-known/caldav"))
}

/// The JMAP autodiscovery URL for `domain` (RFC 8620 §2.2), normally
/// `https://{domain}/.well-known/jmap`. `override_base` (dev harness only) redirects
/// the probe at a local server that can't be reached by domain name.
pub fn jmap_well_known(domain: &Domain, override_base: Option<&Url>) -> Url {
    match override_base {
        Some(base) => base
            .join("/.well-known/jmap")
            .expect("a valid base URL joins a fixed absolute path"),
        None => parse(&format!("https://{domain}/.well-known/jmap")),
    }
}

/// The default HTTPS port: an SRV target on it needs no explicit port in the URL.
const HTTPS_PORT: u16 = 443;

/// The JMAP well-known URL on an SRV-resolved `host:port`, e.g.
/// `https://api.fastmail.com/.well-known/jmap`; where a `_jmap._tcp` SRV record points.
/// `host` is a validated [`Domain`] (never a raw DNS string) and only the standard `443`
/// is elided, so the target is disclosed but the email address still never is.
pub fn jmap_well_known_at(host: &Domain, port: u16) -> Url {
    let authority = if port == HTTPS_PORT {
        host.to_string()
    } else {
        format!("{host}:{port}")
    };
    parse(&format!("https://{authority}/.well-known/jmap"))
}

/// Parses a URL built from a validated [`Domain`] and fixed template text; cannot fail.
fn parse(s: &str) -> Url {
    Url::parse(s).expect("URL templates over a validated domain always parse")
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{
        autoconfig_urls, caldav_well_known, ispdb_url, jmap_well_known, jmap_well_known_at,
        post_mx_urls,
    };
    use crate::types::Domain;

    fn domain() -> Domain {
        Domain::parse("company.example").unwrap()
    }

    #[test]
    fn autoconfig_urls_are_pinned_in_priority_order() {
        let urls: Vec<String> = autoconfig_urls(&domain())
            .iter()
            .map(Url::to_string)
            .collect();
        assert_eq!(
            urls,
            [
                "https://autoconfig.company.example/mail/config-v1.1.xml",
                "https://company.example/.well-known/autoconfig/mail/config-v1.1.xml",
                "http://autoconfig.company.example/mail/config-v1.1.xml",
                "http://company.example/.well-known/autoconfig/mail/config-v1.1.xml",
            ]
        );
    }

    #[test]
    fn ispdb_url_is_pinned() {
        assert_eq!(
            ispdb_url(&domain()).to_string(),
            "https://autoconfig.thunderbird.net/v1.1/company.example"
        );
    }

    #[test]
    fn post_mx_urls_are_https_only_and_pinned() {
        let urls: Vec<String> = post_mx_urls(&domain()).iter().map(Url::to_string).collect();
        assert_eq!(
            urls,
            [
                "https://autoconfig.company.example/mail/config-v1.1.xml",
                "https://autoconfig.thunderbird.net/v1.1/company.example",
            ]
        );
    }

    #[test]
    fn caldav_well_known_is_pinned() {
        assert_eq!(
            caldav_well_known(&domain()).to_string(),
            "https://company.example/.well-known/caldav"
        );
    }

    #[test]
    fn jmap_well_known_is_pinned_and_overridable() {
        assert_eq!(
            jmap_well_known(&domain(), None).to_string(),
            "https://company.example/.well-known/jmap"
        );
        let base = Url::parse("http://127.0.0.1:18080").unwrap();
        assert_eq!(
            jmap_well_known(&domain(), Some(&base)).to_string(),
            "http://127.0.0.1:18080/.well-known/jmap"
        );
    }

    #[test]
    fn jmap_well_known_at_an_srv_target_is_pinned() {
        let target = Domain::parse("api.fastmail.com").unwrap();
        // The standard 443 is elided; a non-standard port is kept.
        assert_eq!(
            jmap_well_known_at(&target, 443).to_string(),
            "https://api.fastmail.com/.well-known/jmap"
        );
        assert_eq!(
            jmap_well_known_at(&target, 8443).to_string(),
            "https://api.fastmail.com:8443/.well-known/jmap"
        );
    }

    #[test]
    fn no_url_ever_carries_an_email_address() {
        // The builders take only a Domain: this test documents the invariant by
        // construction: nothing here can smuggle a local part into a URL.
        let target = Domain::parse("api.provider.example").unwrap();
        let all = autoconfig_urls(&domain())
            .into_iter()
            .chain([ispdb_url(&domain())])
            .chain(post_mx_urls(&domain()))
            .chain([jmap_well_known(&domain(), None)])
            .chain([jmap_well_known_at(&target, 443)])
            .chain([caldav_well_known(&domain())]);
        for url in all {
            assert!(url.query().is_none(), "{url} must carry no query");
            assert!(!url.as_str().contains('@'), "{url} must carry no address");
        }
    }
}
