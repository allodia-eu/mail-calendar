//! Tailored message bodies for the [`crate::showcase`] screenshot dataset — the raw RFC 5322
//! source each showcase message's reading pane renders. Kept apart from the structured data
//! ([`crate::showcase_data`]) so neither file grows past the 500-line limit, and split per
//! locale (one module per catalog locale) for the same reason the seeds are.
//!
//! Everything here is fictional demo content. Bodies are plain HTML (sanitised like any real
//! message before display); the "usage report" is a small `multipart/mixed` carrying a CSV
//! attachment, and the newsletter references a remote image so the reading view demonstrates
//! its remote-image block. A message with no entry here falls back to a preview-only body.

use crate::showcase_data::ShowcaseLocale;

mod de;
mod en;
mod es;
mod fr;
mod it;
mod nl;
mod pt;

/// The tailored raw MIME source for `key` (a message's provider key) in `locale`, or `None`
/// to let the provider synthesize a plain body from the message's preview.
///
/// `now` is only read by the meeting invitation, whose iTIP payload is dated into the current
/// week so the card and the calendar hold describe the same instant.
pub(crate) fn body(
    locale: ShowcaseLocale,
    key: &str,
    now: time::OffsetDateTime,
) -> Option<Vec<u8>> {
    // The invitation is assembled here rather than seven times over: only its prose differs by
    // locale, and its calendar payload is built by `showcase_data` from the same text the hold
    // on the grid uses.
    if key == crate::showcase_data::INVITE_MESSAGE_KEY {
        return Some(invitation_multipart(
            invite_html(locale),
            &crate::showcase_data::invite_ics(locale, now),
        ));
    }
    match locale {
        ShowcaseLocale::En => en::body(key),
        ShowcaseLocale::Nl => nl::body(key),
        ShowcaseLocale::De => de::body(key),
        ShowcaseLocale::Fr => fr::body(key),
        ShowcaseLocale::Es => es::body(key),
        ShowcaseLocale::It => it::body(key),
        ShowcaseLocale::Pt => pt::body(key),
    }
}

/// The invitation mail's human-readable half in `locale` — what a client that ignored the
/// calendar part would show, and what sits under the card.
fn invite_html(locale: ShowcaseLocale) -> &'static str {
    match locale {
        ShowcaseLocale::En => en::INVITE,
        ShowcaseLocale::Nl => nl::INVITE,
        ShowcaseLocale::De => de::INVITE,
        ShowcaseLocale::Fr => fr::INVITE,
        ShowcaseLocale::Es => es::INVITE,
        ShowcaseLocale::It => it::INVITE,
        ShowcaseLocale::Pt => pt::INVITE,
    }
}

/// Wraps an HTML fragment as a complete `text/html` message source.
fn html(inner: &str) -> Vec<u8> {
    format!("Content-Type: text/html; charset=utf-8\r\n\r\n{inner}").into_bytes()
}

/// A `multipart/mixed` source: an HTML body plus a small CSV attachment, so the reading pane
/// shows both a formatted body and a downloadable attachment chip. The caller supplies the
/// localised body, attachment file name, and CSV rows.
fn report_multipart(html_part: &str, file_name: &str, csv: &str) -> Vec<u8> {
    format!(
        "Content-Type: multipart/mixed; boundary=\"scboundary\"\r\n\r\n\
         --scboundary\r\n\
         Content-Type: text/html; charset=utf-8\r\n\r\n\
         {html_part}\r\n\
         --scboundary\r\n\
         Content-Type: text/csv; charset=utf-8; name=\"{file_name}\"\r\n\
         Content-Disposition: attachment; filename=\"{file_name}\"\r\n\r\n\
         {csv}\
         --scboundary--\r\n"
    )
    .into_bytes()
}

/// A meeting invitation in the **iMIP** shape (RFC 6047): a `multipart/alternative` whose second
/// alternative is the iTIP `text/calendar` document.
///
/// An *alternative body part*, deliberately, not an attachment — that is what says "this message
/// **is** an invitation" rather than "here is a calendar file". The core reads the distinction
/// (`InboundScheduling::from_inline_body`) and the reading view acts on it: the payload is
/// consumed into the invitation card and does **not** appear as a paperclip, which is exactly the
/// behaviour the showcase capture is there to show.
fn invitation_multipart(html_part: &str, ics: &str) -> Vec<u8> {
    format!(
        "Content-Type: multipart/alternative; boundary=\"scinvite\"\r\n\r\n\
         --scinvite\r\n\
         Content-Type: text/html; charset=utf-8\r\n\r\n\
         {html_part}\r\n\
         --scinvite\r\n\
         Content-Type: text/calendar; charset=utf-8; method=REQUEST\r\n\r\n\
         {ics}\
         --scinvite--\r\n"
    )
    .into_bytes()
}
