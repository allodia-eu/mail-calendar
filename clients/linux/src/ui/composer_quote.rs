//! The quoted original a reply or forward opens holding, worded in the user's language.
//!
//! The core hands the composer the message it quotes ([`QuoteSource`]) and carries no runtime
//! localisation, so the attribution is worded here, and the date formatted, exactly as the reading
//! header does both. Split from [`super::composer_model`] so each file stays under the 500-line
//! limit; the tests that pin it are that module's.

use mailcal_bindings::{ComposeOpening, QuoteSource, QuoteStyleKind};
use serde_json::{Value, json};

use crate::l10n;

/// A quote's attribution in both styles: the one-line "On … wrote:" and the labelled header
/// block, so the editor can switch a quote between the two.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QuoteWords {
    pub(crate) line: String,
    pub(crate) headers: Vec<(String, String)>,
}

/// The attribution for `quote`. `date` is the original's date as the reading header shows it; a
/// forward swaps the one-line attribution for the forwarded marker.
pub(crate) fn quote_words(quote: &QuoteSource, date: &str, is_forward: bool) -> QuoteWords {
    let line = if is_forward {
        l10n::quote_forwarded().to_owned()
    } else {
        l10n::quote_attribution(date, &quote.from)
    };
    let mut headers = vec![
        (l10n::quote_from().to_owned(), quote.from.clone()),
        (l10n::quote_sent().to_owned(), date.to_owned()),
    ];
    if !quote.to.is_empty() {
        headers.push((l10n::quote_to().to_owned(), quote.to.clone()));
    }
    if !quote.cc.is_empty() {
        headers.push((l10n::quote_cc().to_owned(), quote.cc.clone()));
    }
    headers.push((l10n::quote_subject().to_owned(), quote.subject.clone()));
    QuoteWords { line, headers }
}

/// The `setComposerQuote` payload: a fresh reply's or forward's quoted original, with the text a
/// prefill brought above it. `None` for a stored message, which opens through its own HTML,
/// quote included, and for an original with no body to quote.
///
/// An empty `body_text` leaves the `initial_text` key off entirely, as on every other client: the
/// editor reads an absent key and an empty string differently in where it parks the caret.
pub(crate) fn quote_seed(
    opening: &ComposeOpening,
    words: &QuoteWords,
    style: &QuoteStyleKind,
) -> Option<String> {
    let quote = opening.quote.as_ref()?;
    if !opening.body_html.is_empty() || (quote.body_html.is_empty() && quote.body_plain.is_empty())
    {
        return None;
    }
    let mut payload = json!({
        "style": style_token(style),
        "attribution": attribution(words),
        "body_html": quote.body_html,
        "body_plain": quote.body_plain,
    });
    if !opening.body_text.is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert(
            "initial_text".to_owned(),
            Value::String(opening.body_text.clone()),
        );
    }
    serde_json::to_string(&payload).ok()
}

/// The `completeComposerQuoteAttribution` argument: a reopened message's quote carries only the
/// attribution of the style it was saved in, and the original says what the other one is. `None`
/// for anything but a stored message that still has its original.
pub(crate) fn attribution_completion(
    opening: &ComposeOpening,
    words: &QuoteWords,
) -> Option<String> {
    (!opening.body_html.is_empty() && opening.quote.is_some())
        .then(|| attribution(words).to_string())
}

/// The editor's name for a quote style, as `setComposerQuote` and `setComposerQuoteStyle` take it:
/// the Rust `QuoteStyle` variant names, which the seed carries verbatim.
pub(crate) fn style_token(style: &QuoteStyleKind) -> &'static str {
    match style {
        QuoteStyleKind::Indented => "Indented",
        QuoteStyleKind::LineAndHeader => "LineAndHeader",
    }
}

/// The style of the quote a composer document holds, or `None` when it holds none: what a reopened
/// composer's style picker opens on, since a stored quote keeps the style it was saved in.
pub(crate) fn quote_style_of(document_json: &str) -> Option<QuoteStyleKind> {
    let document: Value = serde_json::from_str(document_json).ok()?;
    let style = document
        .get("blocks")?
        .as_array()?
        .iter()
        .find_map(|block| block.get("Quote"))?
        .get("style")?
        .as_str()?;
    match style {
        "Indented" => Some(QuoteStyleKind::Indented),
        "LineAndHeader" => Some(QuoteStyleKind::LineAndHeader),
        _ => None,
    }
}

fn attribution(words: &QuoteWords) -> Value {
    let headers = words
        .headers
        .iter()
        .map(|(label, value)| json!({ "label": label, "value": value }))
        .collect::<Vec<_>>();
    json!({ "line": words.line, "headers": headers })
}
