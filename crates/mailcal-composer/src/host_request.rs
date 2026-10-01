//! What the editor asks its host for, and what the host answers.
//!
//! The editor runs in a WebView with no general bridge to the app (`docs/composer-security.md`,
//! Gate 2). When it needs something only the host can do well, such as a dialog over the window
//! rather than a panel squeezed into the WebView's own frame, it sends one of these requests over
//! the one named channel each platform registers, and the host answers by running the script
//! [`answer_script`] builds. The vocabulary is closed: a message that is not one of these, or that
//! carries a field this module does not know, is refused here, before any client acts on it.

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::LinkUrl;

/// The longest request accepted, in bytes. A link request carries the selected words, which can be
/// the whole message; the dialog shows a field, not a document.
const MAX_REQUEST_LEN: usize = 64 * 1024;

/// One request from the editor, with the id its answer must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRequest {
    /// Chosen by the editor; the answer is matched to the request by it.
    pub id: u64,
    /// What is asked.
    pub kind: HostRequestKind,
}

/// Everything the editor may ask its host for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HostRequestKind {
    /// Make, change or remove the link on the selection.
    Link(LinkRequest),
}

/// The link editor's starting point: what the selection says and where it points now.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkRequest {
    /// The words the link is shown as: the selected text, or the existing link's.
    pub text: String,
    /// The existing link's target, or empty when the selection carries none.
    pub address: String,
    /// Whether the selection touches a link, so the dialog offers to remove it.
    pub removable: bool,
}

impl fmt::Debug for LinkRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The words and the address are message content: their lengths, never the text.
        f.debug_struct("LinkRequest")
            .field("text_len", &self.text.len())
            .field("address_len", &self.address.len())
            .field("removable", &self.removable)
            .finish()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    id: u64,
    request: HostRequestKind,
}

/// Reads one message from the editor, or `None` when it is not a request this product knows.
#[must_use]
pub fn parse_host_request(message: &str) -> Option<HostRequest> {
    if message.len() > MAX_REQUEST_LEN {
        return None;
    }
    let envelope: Envelope = serde_json::from_str(message).ok()?;
    Some(HostRequest {
        id: envelope.id,
        kind: envelope.request,
    })
}

/// How the user closed the link dialog.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkAnswer {
    /// Link the selection, shown as `text`, to `address`. An empty `text` keeps the words as they
    /// are.
    Apply {
        /// The words the link is shown as.
        text: String,
        /// Where it points.
        address: LinkUrl,
    },
    /// Take the link off the selection.
    Remove,
    /// Leave everything as it was.
    Cancel,
}

impl fmt::Debug for LinkAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Apply { text, address } => f
                .debug_struct("Apply")
                .field("text_len", &text.len())
                .field("address", address)
                .finish(),
            Self::Remove => f.write_str("Remove"),
            Self::Cancel => f.write_str("Cancel"),
        }
    }
}

/// The host's answer to one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAnswer {
    /// The answer to [`HostRequestKind::Link`].
    Link(LinkAnswer),
}

/// The script a host runs in the editor to deliver `answer` to request `id`.
#[must_use]
pub fn answer_script(id: u64, answer: &HostAnswer) -> String {
    // Serializing these types cannot fail: strings, a bool-free enum and a validated URL.
    let json = serde_json::to_string(answer).unwrap_or_else(|_| "null".to_owned());
    // JSON allows U+2028 and U+2029 inside a string and older JavaScript did not; escaping them
    // keeps the script valid on every engine a host might embed.
    let json = json
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    format!("window.answerComposerRequest({id}, {json});")
}

/// What the user typed into the link dialog's address field, as a link target: an address with an
/// allowed scheme is kept, an email address becomes `mailto:`, and a host becomes `https://`.
/// `None` when it cannot be a link, which is when a dialog keeps its OK button disabled.
#[must_use]
pub fn link_address(typed: &str) -> Option<LinkUrl> {
    let value = typed.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some((scheme, _)) = value.split_once(':')
        && is_scheme(scheme)
        // `example.com:8080/x` reads as a scheme; a dot says it is a host.
        && !scheme.contains('.')
    {
        return LinkUrl::new(value);
    }
    if is_email_address(value) {
        return LinkUrl::new(&format!("mailto:{value}"));
    }
    let host = value.strip_prefix("//").unwrap_or(value);
    is_host(host)
        .then(|| LinkUrl::new(&format!("https://{host}")))
        .flatten()
}

/// RFC 3986's `scheme`: a letter, then letters, digits, `+`, `-` or `.`.
fn is_scheme(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `local@domain.tld`, with no `/` or `:` that would make it something else.
fn is_email_address(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    let plain = |part: &str| !part.is_empty() && !part.contains(['@', '/', ':']);
    plain(local) && plain(domain) && has_inner_dot(domain)
}

/// A host name: starts with a letter or digit, and has a dot with something after it before the
/// path begins.
fn is_host(value: &str) -> bool {
    let authority = value.split(['/', '?', '#']).next().unwrap_or_default();
    authority.chars().next().is_some_and(char::is_alphanumeric) && has_inner_dot(authority)
}

/// Whether `value` has a dot that is neither its first nor its last character.
fn has_inner_dot(value: &str) -> bool {
    value
        .char_indices()
        .any(|(at, c)| c == '.' && at > 0 && at + c.len_utf8() < value.len())
}
