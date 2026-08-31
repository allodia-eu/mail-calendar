//! RFC 6068 `mailto:` URI parsing: the shared decode behind "open this mail link in
//! Allodia Mail & Calendar".
//!
//! The OS hands a client an opaque URI that came from a web page, a document, or another
//! app; this module turns it into the exact fields the composer already collects, so a
//! client's whole job is "parse, then open the composer with these". It is deliberately
//! pure and host-free: no allocation of a draft, no send, no network.
//!
//! Three rules are load-bearing, and they are the reason this is shared rather than
//! reimplemented per platform (see `docs/composer-security.md`, Gate 12):
//!
//! - **Only five fields are honoured**; `to`, `cc`, `bcc`, `subject`, `body`. RFC 6068 §6.1 warns
//!   that a URI may name *any* header; everything else (`from`, `reply-to`, `in-reply-to`,
//!   `content-type`, `x-…`) is dropped without comment. A link may suggest a message; it may never
//!   dictate who it appears to come from, how it threads, or how it is encoded.
//! - **Nothing is ever sent.** The result only pre-fills an editable composer the user must still
//!   send themselves. There is no `mailto:` path that puts mail on the wire.
//! - **`+` is a literal plus, not a space.** A `mailto:` URI is percent-encoded (RFC 6068 §2),
//!   *not* `application/x-www-form-urlencoded`: so a generic query-string parser silently corrupts
//!   `?subject=a+b` into `a b`. That is why the query is decoded here by hand rather than through a
//!   URL crate's `query_pairs`.

use core::fmt;

/// The composer prefill decoded from a `mailto:` URI.
///
/// The recipient fields are comma-separated address lists in the same shape the composer's
/// `To`/`Cc`/`Bcc` text fields round-trip (and that `mailcal-app` splits again on send), so a
/// client assigns them straight into its fields. Every field may be empty: a bare `mailto:`
/// is a valid link that means "open a blank composer".
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MailtoPrefill {
    /// The `To` recipients, comma-separated.
    pub to: String,
    /// The `Cc` recipients, comma-separated (may be empty).
    pub cc: String,
    /// The `Bcc` recipients, comma-separated (may be empty).
    pub bcc: String,
    /// The suggested subject, stripped of control characters (may be empty).
    pub subject: String,
    /// The suggested plain-text body, newlines preserved (may be empty).
    pub body: String,
}

impl MailtoPrefill {
    /// Whether the link carried nothing at all: a bare `mailto:` that opens a blank composer.
    ///
    /// A client can use this to decide between "open the composer pre-filled" and "open the
    /// composer as if the user had tapped New message"; both are valid responses to the link.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.to.is_empty()
            && self.cc.is_empty()
            && self.bcc.is_empty()
            && self.subject.is_empty()
            && self.body.is_empty()
    }
}

// A `mailto:` URI is entirely message content; recipients, subject, and body all come
// straight off it. So it is redacted like every other content-bearing type in this crate
// (lengths and counts only), and stays safe to include in a diagnostic log line.
impl fmt::Debug for MailtoPrefill {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MailtoPrefill")
            .field("to_len", &self.to.len())
            .field("cc_len", &self.cc.len())
            .field("bcc_len", &self.bcc.len())
            .field("subject_len", &self.subject.len())
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Parses a `mailto:` URI (RFC 6068) into composer prefill, or `None` if `uri` is not a
/// `mailto:` URI at all.
///
/// The scheme and the header names are matched case-insensitively (`MAILTO:?SUBJECT=Hi` is a
/// valid link). Addresses are taken from the path *and* from any `to` fields, in the order
/// they appear; `cc` and `bcc` accumulate the same way. `subject` and `body` take their
/// **first** occurrence: a repeated field is ignored rather than allowed to overwrite what
/// came before, so appending `&subject=…` to an existing link cannot replace its subject.
///
/// Malformed addresses are dropped individually (the rest of the link still opens), and any
/// header this module does not honour is ignored. Parsing never fails beyond the wrong
/// scheme: a link that is garbage past the scheme opens a blank composer rather than an
/// error the user cannot act on.
#[must_use]
pub fn parse_mailto(uri: &str) -> Option<MailtoPrefill> {
    let rest = strip_scheme(uri)?;
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, query),
        None => (rest, ""),
    };

    let mut prefill = MailtoPrefill::default();
    let mut to = address_list(path);
    let mut cc: Vec<String> = Vec::new();
    let mut bcc: Vec<String> = Vec::new();

    for (name, value) in header_fields(query) {
        match name.as_str() {
            "to" => to.extend(address_list(value)),
            "cc" => cc.extend(address_list(value)),
            "bcc" => bcc.extend(address_list(value)),
            // First occurrence wins; a later duplicate is dropped (see the doc comment).
            "subject" if prefill.subject.is_empty() => {
                prefill.subject = header_text(&percent_decode(value));
            }
            "body" if prefill.body.is_empty() => {
                prefill.body = body_text(&percent_decode(value));
            }
            // Every other header is deliberately ignored; RFC 6068 §6.1. Notably `from`,
            // `reply-to`, and the threading headers: a link does not get to decide those.
            _ => {}
        }
    }

    prefill.to = to.join(", ");
    prefill.cc = cc.join(", ");
    prefill.bcc = bcc.join(", ");
    Some(prefill)
}

/// Strips a case-insensitive `mailto:` scheme, returning everything after the colon.
fn strip_scheme(uri: &str) -> Option<&str> {
    let (scheme, rest) = uri.trim().split_once(':')?;
    scheme.eq_ignore_ascii_case("mailto").then_some(rest)
}

/// Splits a `mailto:` query into `(lowercased header name, still-encoded value)` pairs.
///
/// Splitting happens **before** percent-decoding, so a `%26` or `%3D` inside a value is data
/// rather than a new field; decoding first would let an encoded `&` in a subject inject a
/// `bcc` the user never saw. A field with no `=` is skipped.
fn header_fields(query: &str) -> impl Iterator<Item = (String, &str)> {
    query.split('&').filter_map(|field| {
        let (name, value) = field.split_once('=')?;
        Some((percent_decode(name).to_ascii_lowercase(), value))
    })
}

/// Splits a still-encoded address list on its separating commas, decodes each address, and
/// drops any that isn't a plausible bare addr-spec.
///
/// The split precedes the decode for the same reason as [`header_fields`]: RFC 6068 requires
/// a comma *within* an address to be percent-encoded, so only literal commas separate.
fn address_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(percent_decode)
        .map(|address| address.trim().to_owned())
        .filter(|address| is_addr_spec(address))
        .collect()
}

/// Whether `value` is a plausible bare address, as RFC 6068 requires (an addr-spec; never a
/// display name, angle brackets, or a group).
///
/// This is the header-injection gate on the recipient fields: a CR, LF, or NUL would break out
/// of the header when the draft is assembled, and `<`/`>`/`,`/`"`/whitespace would let one
/// "address" smuggle several. Deliberately strict: a legal-but-vanishingly-rare quoted local
/// part (`"john doe"@example.test`) is rejected rather than special-cased, matching the plain
/// bare-address contract the composer's own recipient fields keep.
fn is_addr_spec(value: &str) -> bool {
    let Some(at) = value.find('@') else {
        return false;
    };
    // An address needs something on both sides of the `@`, and exactly one `@`.
    if at == 0 || at + 1 == value.len() || value.matches('@').count() != 1 {
        return false;
    }
    !value.chars().any(|c| {
        c.is_control() || c.is_whitespace() || matches!(c, ',' | '<' | '>' | '"' | '\\' | ';')
    })
}

/// Normalises a decoded single-line header value (the subject): control characters; CR and LF
/// above all, which would otherwise inject a header, are dropped, and the result is trimmed.
fn header_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Normalises a decoded message body: line endings collapse to `\n` (the composer seeds one
/// paragraph per line) and every other control character is dropped. Newlines and tabs are
/// content here, unlike in a header, so they survive.
fn body_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                // CRLF is one line break, not two.
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\n' | '\t' => out.push(c),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Percent-decodes an RFC 6068 URI component.
///
/// `+` is left alone: this is *not* form encoding (see the module docs). A stray `%` that
/// isn't followed by two hex digits is kept verbatim rather than failing the parse, and an
/// invalid UTF-8 sequence becomes U+FFFD: a link with one bad byte still opens.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
        {
            out.push((high << 4) | low);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The numeric value of one ASCII hex digit, or `None` if `byte` isn't one.
const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
