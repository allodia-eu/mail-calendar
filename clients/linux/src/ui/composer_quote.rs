//! The quoted original a reply or forward opens holding.
//!
//! Split from [`super::composer_model`] so each file stays under the 500-line limit: that one is
//! the composer's own state, this is the one payload the shared editor is seeded with.

use mailcal_bindings::{QuoteStyleKind, ReadingSnapshot};
use serde_json::{Value, json};

use super::{model::OpenedMessage, timestamps};
use crate::l10n;

/// Builds the JSON payload accepted by the shared editor's `setComposerQuote` function.
///
/// `initial_text` pre-fills the lead paragraph above the quote. Only showcase mode supplies one, so
/// a real reply must carry no `initial_text` key at all: the editor treats an absent key and an
/// empty string differently only in where it parks the caret, but writing the key on every reply
/// would put a client-side default into a document the core owns.
pub(crate) fn quote_seed(
    message: &OpenedMessage,
    reading: &ReadingSnapshot,
    style: &QuoteStyleKind,
    is_forward: bool,
    initial_text: Option<&str>,
    zone: &str,
) -> Option<String> {
    if reading.key != message.key {
        return None;
    }
    let body_html = reading.html.as_deref().unwrap_or_default();
    let body_plain = reading.plain.as_deref().unwrap_or_default();
    if body_html.is_empty() && body_plain.is_empty() {
        return None;
    }

    // The reader of this quote is the *recipient*, so the date is localised exactly as the reading
    // header is (docs/timestamps.md). The core emits a UTC instant; sending it raw would put
    // `2026-08-31T05:01:00Z` in their mailbox.
    let sent = timestamps::local_date_time(&message.date, zone);
    let mut headers = vec![
        header(l10n::quote_from(), &message.from),
        header(l10n::quote_sent(), &sent),
    ];
    if !reading.to.is_empty() {
        headers.push(header(l10n::quote_to(), &reading.to));
    }
    if !reading.cc.is_empty() {
        headers.push(header(l10n::quote_cc(), &reading.cc));
    }
    headers.push(header(l10n::quote_subject(), &message.subject));

    let line = if is_forward {
        l10n::quote_forwarded().to_owned()
    } else {
        l10n::quote_attribution(&sent, &message.from)
    };
    let mut payload = json!({
        "style": match style {
            QuoteStyleKind::Indented => "Indented",
            QuoteStyleKind::LineAndHeader => "LineAndHeader",
        },
        "attribution": { "line": line, "headers": headers },
        "body_html": body_html,
        "body_plain": body_plain,
    });
    if let Some(text) = initial_text.filter(|text| !text.is_empty())
        && let Some(object) = payload.as_object_mut()
    {
        object.insert("initial_text".to_owned(), Value::String(text.to_owned()));
    }
    serde_json::to_string(&payload).ok()
}

fn header(label: &str, value: &str) -> Value {
    json!({ "label": label, "value": value })
}
