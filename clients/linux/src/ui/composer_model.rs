//! Pure request state for the Linux composer host: what a composer opens with, built from the
//! core's answer in one place, and the heading it carries. The quoted original's wording is
//! [`super::composer_quote`].

pub(crate) use mailcal_bindings::ComposeKind;
use mailcal_bindings::{ComposeOpening, QuoteStyleKind};
use serde_json::json;

use super::{
    composer_quote::{attribution_completion, quote_seed, quote_words},
    reader::ComposerHost,
    timestamps,
};
use crate::l10n;

/// Everything fixed when a single composer session opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposeContext {
    /// What the composer is doing: its heading, and the signature slot it seeds from.
    pub(crate) kind: ComposeKind,
    /// Where this draft is being written: the pane, or a window of its own. Fixed when the
    /// composer opens like everything else here, and it is what Cancel, Send and the window's
    /// close all report back against (`docs/reading-window.md`).
    pub(crate) host: ComposerHost,
    pub(crate) initial_to: String,
    pub(crate) initial_cc: String,
    pub(crate) initial_bcc: String,
    pub(crate) subject: String,
    /// A stored message's text, or the text a prefill brought.
    pub(crate) initial_body: Option<String>,
    /// The HTML of a message this composer reopens (a resumed draft, or one moved back out of
    /// the Outbox), which the editor reads back into its document; `initial_body` is then that
    /// message's text, for when it has none.
    pub(crate) stored_html: Option<String>,
    /// The `setComposerQuote` payload of a fresh reply or forward.
    pub(crate) quote: Option<String>,
    /// The `completeComposerQuoteAttribution` argument of a reopened message that quotes one.
    pub(crate) quote_attribution: Option<String>,
    /// Whether the message carries a quoted original, fresh or reopened: what the per-message
    /// style picker is for.
    pub(crate) has_quote: bool,
    pub(crate) initial_from: Option<String>,
    /// Whether this composer seeds and offers the account's signature. A stored message, and an
    /// assistant's draft, already carry their own.
    pub(crate) seeds_signature: bool,
    /// The composition the core joined to this composer as it opened: what every save, the
    /// discard, the close and the send name (`docs/drafts.md`).
    pub(crate) composition: String,
    /// Files the composer opens holding: the ones the core staged, and a share's on top.
    pub(crate) files: Vec<PickedFile>,
    /// How many of `files` the core staged: a forward's originals or a stored message's own,
    /// still in the mailbox, so closing over them unchanged loses nothing. A share's files are
    /// counted from the start, because the user chose them.
    pub(crate) opening_files: usize,
    /// Whether a forward's originals could not be staged, so the composer opens saying so.
    pub(crate) attachments_failed: bool,
}

impl ComposeContext {
    /// The one way a composer's context is built, whichever way it was reached.
    ///
    /// The quote's attribution is worded here, in the user's language and with the original's
    /// date as the reading header shows it: for a fresh reply or forward it seeds the quote, and
    /// for a reopened one it completes the style the stored quote does not carry.
    pub(crate) fn from_opening(
        opening: ComposeOpening,
        host: ComposerHost,
        style: &QuoteStyleKind,
        zone: &str,
    ) -> Self {
        let words = opening.quote.as_ref().map(|quote| {
            quote_words(
                quote,
                &timestamps::local_date_time(&quote.date, zone),
                opening.kind == ComposeKind::Forward,
            )
        });
        let quote = words
            .as_ref()
            .and_then(|words| quote_seed(&opening, words, style));
        let quote_attribution = words
            .as_ref()
            .and_then(|words| attribution_completion(&opening, words));
        let has_quote = opening.quote.is_some();
        let ComposeOpening {
            composition,
            kind,
            account,
            to,
            cc,
            bcc,
            subject,
            body_html,
            body_text,
            attachments,
            attachments_failed,
            seeds_signature,
            ..
        } = opening;
        let files: Vec<PickedFile> = attachments
            .into_iter()
            .map(|file| PickedFile {
                path: file.path,
                file_name: file.file_name,
                media_type: file.media_type,
            })
            .collect();
        Self {
            kind,
            host,
            initial_to: to,
            initial_cc: cc,
            initial_bcc: bcc,
            subject,
            initial_body: (!body_text.is_empty()).then_some(body_text),
            stored_html: (!body_html.is_empty()).then_some(body_html),
            quote,
            quote_attribution,
            has_quote,
            initial_from: Some(account).filter(|account| !account.is_empty()),
            seeds_signature,
            composition,
            opening_files: files.len(),
            files,
            attachments_failed,
        }
    }

    /// Whether this composer opens with the caret in the message body rather than in To.
    ///
    /// A reply/forward is already addressed, so writing is the only thing left to do and the body
    /// takes it; a new message's To is empty and is where the user has to begin. A mail link is the
    /// exception among new messages; it supplied the recipient, so the body is the place there
    /// too. One predicate rather than two flags, because exactly one of the two may be focused
    /// (docs/contacts.md §4).
    pub(crate) fn opens_in_body(&self) -> bool {
        self.kind != ComposeKind::New || !self.initial_to.trim().is_empty()
    }
}

/// A fresh composition id, minted as a composer opens.
///
/// A counter rather than a UUID, because a composition is **session state**: the core holds the
/// record joining it to a stored key only while the composer is open, and forgets it on close
/// (`docs/drafts.md`). So the id has to be unique among the composers of one process and nothing
/// more, and this client's other window ids are minted the same way.
pub(crate) fn new_composition() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "composer-{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// The one editor call that seeds the body, encoded as JavaScript data rather than code: a
/// reopened message's HTML, else a fresh quote, else the text a prefill brought.
pub(crate) fn body_seed_script(request: &ComposeContext) -> Option<String> {
    let text = request.initial_body.as_deref().unwrap_or_default();
    if let Some(html) = request
        .stored_html
        .as_deref()
        .filter(|html| !html.trim().is_empty())
    {
        return Some(format!(
            "window.setComposerBody({});",
            json!({ "html": html, "text": text })
        ));
    }
    if let Some(quote) = &request.quote {
        return Some(format!("window.setComposerQuote({});", json!(quote)));
    }
    (!text.is_empty()).then(|| format!("window.setPlainText({});", json!(text)))
}

/// The editor call that completes a reopened quote's attribution, sent after the signature.
pub(crate) fn attribution_script(request: &ComposeContext) -> Option<String> {
    request.quote_attribution.as_ref().map(|attribution| {
        format!(
            "window.completeComposerQuoteAttribution({});",
            json!(attribution)
        )
    })
}

/// The heading a composer of `kind` carries.
pub(crate) fn compose_title(kind: ComposeKind) -> &'static str {
    match kind {
        ComposeKind::New => l10n::compose_title_new(),
        ComposeKind::Reply => l10n::action_reply(),
        ComposeKind::ReplyAll => l10n::action_reply_all(),
        ComposeKind::Forward => l10n::action_forward(),
    }
}

/// Metadata for one native file selected for an outgoing message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickedFile {
    pub(crate) path: String,
    pub(crate) file_name: String,
    pub(crate) media_type: String,
}

/// Editor and header values captured by the Send action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposerSubmission {
    pub(crate) request: ComposeContext,
    pub(crate) to: String,
    pub(crate) cc: String,
    pub(crate) bcc: String,
    pub(crate) subject: String,
    pub(crate) document_json: String,
    pub(crate) files: Vec<PickedFile>,
    pub(crate) from: Option<String>,
}

#[cfg(test)]
#[path = "composer_model_tests.rs"]
mod tests;
