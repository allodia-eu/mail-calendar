//! The quoted-original block embedded below a reply or forward.
//!
//! The product core seeds reply/forward composing with one [`Quote`] block carrying the
//! original message; its **already-sanitised** HTML (and plain-text) plus the attribution
//! to show above it. Two [`QuoteStyle`]s render the same quote differently: `Indented` puts it
//! in a left-bordered blockquote under a one-line attribution; `LineAndHeader` divides it off
//! with a rule and a `From:/Sent:/To:/Subject:` header block at full width. Both renderings are
//! carried (the one-line `attribution.line` and the `attribution.headers`) so a host can flip
//! the style in the composer without asking the core to rebuild the quote.
//!
//! Security: `body_html` is an inert, pre-sanitised fragment. The core sanitises it when it
//! seeds the quote **and** re-sanitises it on submit (the WebView editor round-trips it and is
//! not trusted); the composer emits it verbatim. See `docs/composer-security.md`.

use core::fmt;

use serde::{Deserialize, Serialize};

/// The house style a quoted original is rendered in. The variant names are also the wire
/// tokens: they serialize verbatim into the `Quote` block a host's editor round-trips.
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteStyle {
    /// The original indented inside a left-bordered `<blockquote>`, introduced by a single
    /// "On `<date>`, `<sender>` wrote:" line; the reply text sits above it.
    #[default]
    Indented,
    /// A dividing rule, then a `From:/Sent:/To:/Subject:` header block, the original
    /// following at full width (no indentation).
    LineAndHeader,
}

impl fmt::Debug for QuoteStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Indented => "Indented",
            Self::LineAndHeader => "LineAndHeader",
        })
    }
}

/// One labelled line of a [`QuoteStyle::LineAndHeader`] quote header, e.g.
/// `From: Alice <a@x.test>`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteHeader {
    /// The field label, already localised by the core (e.g. `From`, `Sent`, `To`).
    pub label: String,
    /// The field value (a formatted address list, date, or subject).
    pub value: String,
}

impl fmt::Debug for QuoteHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuoteHeader")
            .field("label", &self.label)
            .field("value_len", &self.value.len())
            .finish()
    }
}

/// The attribution shown above a quoted original. Carries **both** renderings so a host can
/// switch [`QuoteStyle`] without rebuilding the quote: `line` is the one-liner
/// [`QuoteStyle::Indented`] shows, `headers` the block [`QuoteStyle::LineAndHeader`] shows.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteAttribution {
    /// The one-line attribution, e.g. `On 30 Jun 2026 at 14:03, Alice <a@x.test> wrote:`.
    pub line: String,
    /// The header lines (From/Sent/To/Cc/Subject), in display order.
    #[serde(default)]
    pub headers: Vec<QuoteHeader>,
}

impl fmt::Debug for QuoteAttribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuoteAttribution")
            .field("line_len", &self.line.len())
            .field("headers_len", &self.headers.len())
            .finish()
    }
}

/// A quoted original message embedded below a reply or forward.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    /// Which style to render the quote in.
    pub style: QuoteStyle,
    /// The attribution shown above the quoted body.
    pub attribution: QuoteAttribution,
    /// The original's **already-sanitised** inert HTML fragment (see the module note). The
    /// composer emits this verbatim; the core guarantees it is sanitised at seed and on submit.
    pub body_html: String,
    /// The original's plain-text body, for the outgoing message's `text/plain` part. Empty
    /// when the original carried no text part.
    #[serde(default)]
    pub body_plain: String,
}

impl fmt::Debug for Quote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Quote")
            .field("style", &self.style)
            .field("attribution", &self.attribution)
            .field("body_html_len", &self.body_html.len())
            .field("body_plain_len", &self.body_plain.len())
            .finish()
    }
}
