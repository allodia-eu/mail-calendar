//! FFI surface for opening a `mailto:` link (RFC 6068) in the composer.
//!
//! The OS hands a client an opaque URI; the client asks Rust what it means and opens its
//! composer with the answer. The decode itself lives in `mailcal_composer::mailto` so every
//! platform inherits the same header allowlist and injection defences; see
//! `docs/composer-security.md`, Gate 12.

use mailcal_composer::parse_mailto;

/// What a `mailto:` link asks the composer to be pre-filled with.
///
/// `to`/`cc`/`bcc` are comma-separated address lists in the same shape the composer's
/// recipient fields already round-trip, so a host assigns them straight into its fields (and
/// into [`crate::Recipients`] on submit). Every field is editable by the user afterwards, and
/// **nothing here is ever sent on its own**: a link pre-fills a composer, it does not send
/// mail.
#[derive(uniffi::Record)]
pub struct MailtoPrefill {
    /// The `To` recipients, comma-separated (may be empty).
    pub to: String,
    /// The `Cc` recipients, comma-separated (may be empty).
    pub cc: String,
    /// The `Bcc` recipients, comma-separated (may be empty).
    pub bcc: String,
    /// The suggested subject (may be empty).
    pub subject: String,
    /// The suggested plain-text body, one paragraph per line (may be empty).
    pub body: String,
}

/// Parses a `mailto:` URI into composer prefill, or returns `None` when `uri` is not a
/// `mailto:` URI, which is a host's cue to ignore the launch rather than open a composer.
///
/// Only `to`, `cc`, `bcc`, `subject`, and `body` are honoured; every other header a URI may
/// name (RFC 6068 §6.1; `from`, `reply-to`, threading, `content-type`, `x-…`) is dropped, and
/// addresses that could break out of a header are discarded individually. A bare `mailto:`
/// yields all-empty fields, meaning "open a blank composer".
///
/// Pure and synchronous: no store, no network, no account needed, so a host may call it
/// before the core has finished connecting.
#[must_use]
#[uniffi::export]
pub fn parse_mailto_uri(uri: String) -> Option<MailtoPrefill> {
    parse_mailto(&uri).map(|prefill| MailtoPrefill {
        to: prefill.to,
        cc: prefill.cc,
        bcc: prefill.bcc,
        subject: prefill.subject,
        body: prefill.body,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_mailto_uri;

    #[test]
    fn a_mail_link_crosses_the_ffi_as_composer_prefill() {
        let prefill = parse_mailto_uri(
            "mailto:alice@example.test?subject=Hi%20there&cc=c@example.test".to_owned(),
        )
        .expect("a mailto URI");
        assert_eq!(prefill.to, "alice@example.test");
        assert_eq!(prefill.cc, "c@example.test");
        assert_eq!(prefill.subject, "Hi there");
        assert_eq!(prefill.body, "");
    }

    #[test]
    fn a_non_mail_link_is_none_so_the_host_ignores_the_launch() {
        assert!(parse_mailto_uri("https://allodia.eu".to_owned()).is_none());
    }

    #[test]
    fn a_spoofed_from_never_reaches_the_host() {
        // The allowlist is enforced in the shared core, so this holds on every platform; a
        // client cannot widen it by passing the URI through differently.
        let prefill = parse_mailto_uri(
            "mailto:alice@example.test?from=spoof@evil.test&bcc=snoop@evil.test".to_owned(),
        )
        .expect("a mailto URI");
        assert_eq!(prefill.to, "alice@example.test");
        // `from` is dropped; `bcc` is genuinely honoured (RFC 6068 lists it among the safe
        // fields), which is precisely why a host must OPEN its Cc/Bcc row when this is
        // non-empty: a recipient the user cannot see is one they cannot remove.
        assert_eq!(prefill.bcc, "snoop@evil.test");
    }
}
