//! What the editor asks its host for, and what the host answers.
//!
//! The editor runs in a WebView with no general bridge to the app (`docs/composer-security.md`,
//! Gate 2). When it needs something only the host can do well, such as a dialog over the window
//! rather than a panel squeezed into the WebView's own frame, it sends one of these requests over
//! the one named channel each platform registers, and the host answers by running the script
//! [`answer_script`] builds. The vocabulary is closed: a message that is not one of these, or that
//! carries a field this module does not know, is refused here, before any client acts on it.

use core::fmt;

use engine_html::link::without_number_spaces;
use serde::{Deserialize, Serialize};

use crate::{
    LinkUrl,
    page::{ColoursRequest, PageScheme},
};

/// The longest request accepted, in bytes. A link request carries the selected words, which can be
/// the whole message; the dialog shows a field, not a document.
const MAX_REQUEST_LEN: usize = 64 * 1024;

/// The longest colours request accepted, in bytes. It carries the colour attributes and style
/// sheets of a quoted original, and a forwarded newsletter's sheets alone can pass the general
/// limit. The editor sends them in batches under this.
pub const MAX_COLOURS_REQUEST_LEN: usize = 1024 * 1024;

/// One request from the editor, with the id its answer must carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRequest {
    /// Chosen by the editor; the answer is matched to the request by it.
    pub id: u64,
    /// What is asked.
    pub kind: HostRequestKind,
}

/// Everything the editor may ask its host for, or tell it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HostRequestKind {
    /// Make, change or remove the link on the selection.
    Link(LinkRequest),
    /// The body's undo history changed. A notice rather than a request: nothing answers it.
    History(HistorySteps),
    /// Where the caret is, in its framed mode (`docs/composer-layout.md`, rule 6). A notice.
    Caret(CaretSpan),
    /// How far the message is scrolled inside its box, in its framed mode
    /// (`docs/composer-layout.md`, rule 2). A notice.
    Scroll(MessageScroll),
    /// What the reply above the signature and the quote holds, for a drafted reply
    /// (`docs/ai.md`, rules 21 to 23). A notice, handed to the core rather than read by a client.
    ReplyDraft(ReplyDraftReport),
    /// How each piece of the message looks on the page it is written on
    /// (`docs/reading-dark-mode.md`, "Writing in a dark app"). The core answers it, not a client.
    Colours(ColoursRequest),
    /// The person pressed the toolbar's sun or moon: the page they asked for. A notice the host
    /// acts on by setting the one dark-messages preference (`docs/reading-dark-mode.md`, rule 2).
    Background(BackgroundChoice),
}

/// The page the person asked for with the toolbar's sun or moon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundChoice {
    /// The page they asked for.
    pub scheme: PageScheme,
}

/// What the text above the signature and the quote holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadText {
    /// Nothing: no text and no picture.
    Empty,
    /// The draft the editor put there, untouched.
    Draft,
    /// Anything else: what the person wrote, or a draft they edited.
    Written,
}

/// The editor's account of the reply, sent after each change to it. Carries no content: the
/// draft's gaps are named by their place in the list the editor was given.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyDraftReport {
    /// The draft the editor was last given, by the number the core gave it; zero before any.
    pub serial: u64,
    /// What the text above the signature and the quote holds.
    pub lead: LeadText,
    /// Whether that draft is in the body: false once its step is undone, true again on a redo.
    pub inserted: bool,
    /// The draft's gaps still in the reply, by their place in its list.
    pub left: Vec<u32>,
    /// How many times the text above the signature and the quote has changed. Never goes down.
    pub edits: u64,
}

/// How far the message is scrolled inside its box, and where its text ends, in whole CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageScroll {
    /// The distance from the message's top; zero at its top.
    pub px: u32,
    /// The bottom of the message's last line, in the page's coordinates as it is scrolled now:
    /// past the page's height while there is more of the message below its box.
    pub end: u32,
}

/// The caret's top and bottom edges, in whole CSS pixels from the top of the editor's page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaretSpan {
    /// The caret's top edge.
    pub top: u32,
    /// The caret's bottom edge, never above its top.
    pub bottom: u32,
}

/// Whether the body's history has a step to take back and one to put back, for a host whose menu
/// or gesture asks that synchronously and so cannot ask the page (`docs/undo.md`, rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistorySteps {
    /// A step can be undone.
    pub undo: bool,
    /// A step can be redone.
    pub redo: bool,
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
    if message.len() > MAX_COLOURS_REQUEST_LEN {
        return None;
    }
    let envelope: Envelope = serde_json::from_str(message).ok()?;
    if message.len() > MAX_REQUEST_LEN && !matches!(envelope.request, HostRequestKind::Colours(_)) {
        return None;
    }
    if let HostRequestKind::Caret(caret) = &envelope.request
        && caret.bottom < caret.top
    {
        return None;
    }
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
    /// The answer to [`HostRequestKind::Colours`]: one entry per piece, in its order, `None` where
    /// the piece is shown as written.
    Colours(Vec<Option<String>>),
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
    let value = without_number_spaces(typed.trim());
    let value = value.as_ref();
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
