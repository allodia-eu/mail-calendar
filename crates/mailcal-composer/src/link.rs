//! The address a linked run points at, and the schemes any link in this product may carry.
//!
//! One allowlist for both directions: what the composer lets a message link to, what the reading
//! view's sanitiser keeps on an `<a href>`, and what a clicked link may be handed to the OS for.
//! A link the sanitiser strips can never be clicked, and a link we send should be one our own
//! reader would open, so the three cannot be allowed to disagree.

use core::fmt;

use serde::{Deserialize, Serialize};

/// The URL schemes a link may carry. Deliberately strict: never `javascript:`, `data:`, `file:`
/// or a custom application scheme.
pub const LINK_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// The longest link the composer sends. Far above any real address, and low enough that a
/// document cannot inflate a message through one attribute.
const MAX_LINK_LEN: usize = 2048;

/// Whether `url` starts with one of [`LINK_SCHEMES`].
#[must_use]
pub fn has_link_scheme(url: &str) -> bool {
    scheme_of(url).is_some_and(|scheme| LINK_SCHEMES.contains(&scheme.as_str()))
}

/// The lowercased URL scheme (the part before the first `:`), if `url` starts with a
/// syntactically valid RFC 3986 scheme; `None` for a relative URL or anything not in `scheme:`
/// form. Byte-safe on hostile input: `split_once` and char iteration never slice mid-codepoint.
fn scheme_of(url: &str) -> Option<String> {
    let (scheme, _) = url.trim().split_once(':')?;
    let mut chars = scheme.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// A validated link target: one of [`LINK_SCHEMES`], something after the scheme, no whitespace or
/// control characters, and no longer than 2048 bytes.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct LinkUrl(String);

impl LinkUrl {
    /// Creates a link target, or `None` when `value` is not one this product sends.
    ///
    /// The value is placed in an `href="…"` attribute (escaped) and shown in the plain-text part,
    /// so whitespace is refused rather than trimmed away inside it: a URL holding a line break is
    /// not the URL the user saw.
    #[must_use]
    pub fn new(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.len() > MAX_LINK_LEN
            || value.chars().any(|c| c.is_whitespace() || c.is_control())
            || !has_link_scheme(value)
        {
            return None;
        }
        let (_, rest) = value.split_once(':')?;
        let rest = rest.trim_start_matches('/');
        (!rest.is_empty()).then(|| Self(value.to_owned()))
    }

    /// The target as the user will see it in the plain-text part.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for LinkUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Where the user's message points is message content: its length, never the address.
        f.debug_tuple("LinkUrl").field(&self.0.len()).finish()
    }
}

/// Deserializes an optional link, mapping anything [`LinkUrl`] refuses to `None`.
///
/// Lenient for the reason colour is (`crate::color::deserialize_color`): the run still sends as
/// text, which is a smaller loss than refusing the message.
pub(crate) fn deserialize_link<'de, D>(deserializer: D) -> Result<Option<LinkUrl>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.as_deref().and_then(LinkUrl::new))
}

#[cfg(test)]
mod tests {
    use super::{LinkUrl, has_link_scheme};

    #[test]
    fn accepts_web_and_mail_links() {
        for good in [
            "https://example.com",
            "http://example.com/a?b=c&d=e#f",
            "HTTPS://Example.com",
            "mailto:someone@example.com",
            "  https://example.com  ",
        ] {
            assert!(LinkUrl::new(good).is_some(), "expected {good:?} to be kept");
        }
    }

    #[test]
    fn refuses_every_other_scheme_and_malformed_value() {
        for bad in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,<script>x</script>",
            "file:///etc/passwd",
            "myapp://open",
            "example.com",
            "/relative",
            "https://",
            "mailto:",
            "https://exa mple.com",
            "https://example.com/\nBcc: x",
            "",
        ] {
            assert!(
                LinkUrl::new(bad).is_none(),
                "expected {bad:?} to be refused"
            );
        }
    }

    #[test]
    fn refuses_an_oversized_link() {
        let long = format!("https://example.com/{}", "a".repeat(2048));
        assert!(LinkUrl::new(&long).is_none());
    }

    #[test]
    fn the_scheme_check_is_case_insensitive_and_byte_safe() {
        assert!(has_link_scheme("MAILTO:a@b.example"));
        assert!(!has_link_scheme("ht tp://x"));
        assert!(!has_link_scheme("é:x"));
        assert!(!has_link_scheme("no scheme"));
    }
}
