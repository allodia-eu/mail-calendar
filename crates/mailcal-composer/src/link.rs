//! The address a linked run points at.
//!
//! Which schemes a link may carry is the engine's one allowlist (`engine_html::link`): what the
//! composer lets a message link to, what the reading view's sanitiser keeps on an `<a href>`, and
//! what a clicked link may be handed to the OS for, so the three cannot disagree.

use core::fmt;

use engine_html::link::is_link_target;
use serde::{Deserialize, Serialize};

/// The longest link the composer sends. Far above any real address, and low enough that a
/// document cannot inflate a message through one attribute.
const MAX_LINK_LEN: usize = 2048;

/// A validated link target: one [`is_link_target`] accepts, something after the scheme, no
/// whitespace or control characters, and no longer than 2048 bytes.
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
            || !is_link_target(value)
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
    use super::LinkUrl;

    #[test]
    fn accepts_web_mail_and_telephone_links() {
        for good in [
            "https://example.com",
            "http://example.com/a?b=c&d=e#f",
            "HTTPS://Example.com",
            "mailto:someone@example.com",
            "tel:+15032223333",
            "tel:+44-20-7946-0958;ext=1234",
            "TEL:+15032223333",
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
            "sms:+15032223333",
            "callto:+15032223333",
            "sip:ada@example.test",
            "whatsapp://send?phone=15032223333",
            "example.com",
            "/relative",
            "https://",
            "mailto:",
            "tel:",
            "tel:+1 503 222 3333",
            "tel:*21*+31612345678#",
            "tel:112",
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
}
