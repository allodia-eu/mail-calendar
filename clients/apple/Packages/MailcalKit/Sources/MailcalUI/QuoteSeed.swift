// Builds the seed for the composer's quoted original on reply/forward. The quoted body is the
// reading view's already-sanitised HTML (and plain text) for the open message; the attribution
// is localised here, the Rust core carries no runtime localisation, so, like date display, the
// client formats it (L10n + the device's formatted date already on `OpenedMessage`). The shape
// matches the Rust composer's `Block::Quote` so it round-trips through the shared composer; the
// core re-sanitises the body on submit (docs/composer-security.md, Gate 10).

import Foundation
import MailcalBindings

/// A worked example of a quoted original, for the settings screen to render so the user can see what
/// each style looks like instead of guessing from its name. Built by `ComposerQuote.example` from
/// the *same* catalog keys as a real quote, so the example cannot drift from the real thing.
struct QuoteExample {
    /// The one-line attribution the indented style shows.
    let line: String
    /// The labelled From/Sent/To/Subject rows the line-and-header style shows, in display order.
    let headers: [(label: String, value: String)]
    /// The quoted message body.
    let body: String
}

enum ComposerQuote {
    /// The seed JSON for `setComposerQuote`, or `nil` when there is nothing to quote yet (the
    /// body hasn't loaded for this message). `isForward` swaps the one-line attribution for a
    /// "Forwarded message" marker; the header block is the same either way.
    /// `initialText` pre-fills the paragraph above the quote; only showcase mode passes it, and
    /// the editor assigns it as text, never markup (`docs/composer-security.md`, Gate 11).
    static func seedJSON(
        style: QuoteStyleKind,
        message: OpenedMessage,
        reading: ReadingSnapshot?,
        isForward: Bool,
        zone: String,
        initialText: String? = nil
    ) -> String? {
        guard let reading, reading.key == message.key else { return nil }
        let bodyHTML = reading.html ?? ""
        let bodyPlain = reading.plain ?? ""
        guard !(bodyHTML.isEmpty && bodyPlain.isEmpty) else { return nil }

        // The reader of this quote is the *recipient*, so the date is localised exactly as the
        // reading header is (`docs/timestamps.md`). The core emits a UTC instant; sending it raw
        // would put `2026-08-31T05:01:00Z` in their mailbox.
        let sent = localDateTime(message.date, in: zone)
        let line = isForward
            ? L10n.quote_forwarded()
            : L10n.quote_attribution(date: sent, sender: message.from)

        var headers: [[String: String]] = [
            ["label": L10n.quote_from(), "value": message.from],
            ["label": L10n.quote_sent(), "value": sent],
        ]
        if !reading.to.isEmpty {
            headers.append(["label": L10n.quote_to(), "value": reading.to])
        }
        if !reading.cc.isEmpty {
            headers.append(["label": L10n.quote_cc(), "value": reading.cc])
        }
        headers.append(["label": L10n.quote_subject(), "value": message.subject])

        var payload: [String: Any] = [
            "style": token(style),
            "attribution": ["line": line, "headers": headers],
            "body_html": bodyHTML,
            "body_plain": bodyPlain,
        ]
        if let initialText, !initialText.isEmpty {
            payload["initial_text"] = initialText
        }
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8)
        else {
            return nil
        }
        return json
    }

    /// The style token the editor's `setComposerQuote`/`setComposerQuoteStyle` expect. These are
    /// the Rust `QuoteStyle` variant names, which serialize verbatim into the seed JSON, a rename
    /// on either side has to move both (mailcal-composer pins them with a test).
    static func token(_ style: QuoteStyleKind) -> String {
        style == .lineAndHeader ? "LineAndHeader" : "Indented"
    }

    /// Whether a composer shows its per-message style picker. Both have to hold: the message must
    /// carry a quoted original (a new message has nothing to style), and the user must have opted
    /// into per-message styling in Settings, off by default, so an ordinary reply just uses the
    /// app default and the composer stays uncluttered.
    static func showsStylePicker(hasQuote: Bool, perMessage: Bool) -> Bool {
        hasQuote && perMessage
    }

    /// The sample quote the settings screen renders under each style. Only the sender, date, subject
    /// and body are stand-ins: the attribution line and the header *labels* come from the very keys
    /// `seedJSON` uses above, so what settings shows is what a real reply produces.
    static func example() -> QuoteExample {
        let sender = L10n.quote_preview_sender()
        let date = L10n.quote_preview_date()
        return QuoteExample(
            line: L10n.quote_attribution(date: date, sender: sender),
            headers: [
                (label: L10n.quote_from(), value: sender),
                (label: L10n.quote_sent(), value: date),
                (label: L10n.quote_to(), value: L10n.quote_preview_to()),
                (label: L10n.quote_subject(), value: L10n.quote_preview_subject()),
            ],
            body: L10n.quote_preview_body()
        )
    }
}
