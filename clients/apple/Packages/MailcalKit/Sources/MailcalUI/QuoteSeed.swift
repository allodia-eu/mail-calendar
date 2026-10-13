// The words of a quoted original's attribution, in the user's language. The core hands the
// composer the message it quotes (`ComposeOpening.quote`); the Rust core carries no runtime
// localisation, so, like date display, the client words it. `ComposeOpeningSeed` turns the words
// into what the editor is seeded with.

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
    /// A quote's attribution in both styles, built from the message it quotes. `isForward` swaps
    /// the one-line attribution for a "Forwarded message" marker; the header block is the same
    /// either way. `date` is the original's date as the reading header shows it.
    static func words(_ quote: QuoteSource, date: String, isForward: Bool) -> QuoteWords {
        let line = isForward
            ? L10n.quote_forwarded()
            : L10n.quote_attribution(date: date, sender: quote.from)
        var headers: [(label: String, value: String)] = [
            (label: L10n.quote_from(), value: quote.from),
            (label: L10n.quote_sent(), value: date),
        ]
        if !quote.to.isEmpty {
            headers.append((label: L10n.quote_to(), value: quote.to))
        }
        if !quote.cc.isEmpty {
            headers.append((label: L10n.quote_cc(), value: quote.cc))
        }
        headers.append((label: L10n.quote_subject(), value: quote.subject))
        return QuoteWords(line: line, headers: headers)
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
    /// `words` uses above, so what settings shows is what a real reply produces.
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
