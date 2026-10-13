// The words of a quoted original's attribution, in the user's language. The core hands the
// composer the message it quotes (`ComposeOpening.quote`); the Rust core carries no runtime
// localisation, so, like date display, the client words it. ComposeOpeningSeed.kt turns the words
// into what the editor is seeded with. The Android twin of macOS's QuoteSeed.swift.
package eu.allodia.mailcal

import android.content.Context
import uniffi.mailcal_bindings.QuoteSource
import uniffi.mailcal_bindings.QuoteStyleKind

// A worked example of a quoted original, for the settings screen to render so the user can see
// what each style looks like instead of guessing from its name. Assembled by `ComposerQuote.example`
// from the *same* catalog keys as a real quote, so the example cannot drift from the real thing.
internal data class QuoteExample(
    // The one-line attribution the indented style shows.
    val line: String,
    // The labelled From/Sent/To/Subject rows the line-and-header style shows, in display order.
    val headers: List<Pair<String, String>>,
    // The quoted message body.
    val body: String,
)

internal object ComposerQuote {
    // A quote's attribution in both styles, built from the message it quotes. `isForward` swaps
    // the one-line attribution for a "Forwarded message" marker; the header block is the same
    // either way. `dateText` is the original's date as the reading header shows it.
    fun words(ctx: Context, quote: QuoteSource, dateText: String, isForward: Boolean): QuoteWords {
        val line = if (isForward) {
            L10n.quote_forwarded(ctx)
        } else {
            L10n.quote_attribution(ctx, dateText, quote.from)
        }
        val headers = buildList {
            add(L10n.quote_from(ctx) to quote.from)
            add(L10n.quote_sent(ctx) to dateText)
            if (quote.to.isNotEmpty()) add(L10n.quote_to(ctx) to quote.to)
            if (quote.cc.isNotEmpty()) add(L10n.quote_cc(ctx) to quote.cc)
            add(L10n.quote_subject(ctx) to quote.subject)
        }
        return QuoteWords(line, headers)
    }

    // The style token the editor's `setComposerQuote`/`setComposerQuoteStyle` expect. These are the
    // Rust `QuoteStyle` variant names, which serialize verbatim into the seed JSON, a rename on
    // either side has to move both (mailcal-composer pins them with a test).
    fun token(style: QuoteStyleKind): String =
        if (style == QuoteStyleKind.LINE_AND_HEADER) "LineAndHeader" else "Indented"

    // Whether a composer shows its per-message style picker. Both have to hold: the message must
    // actually carry a quoted original (a new message has nothing to style), and the user must have
    // opted into per-message styling in Settings. Off by default, so the ordinary reply just uses
    // the app default and the composer stays uncluttered.
    fun showsStylePicker(hasQuote: Boolean, perMessage: Boolean): Boolean = hasQuote && perMessage

    // The sample quote the settings screen renders under each style. Only the sender, date, subject
    // and body are stand-ins: the attribution line and the header *labels* come from the very keys
    // `words` uses above, so what settings shows is what a real reply produces.
    fun example(ctx: Context): QuoteExample {
        val sender = L10n.quote_preview_sender(ctx)
        val date = L10n.quote_preview_date(ctx)
        return QuoteExample(
            line = L10n.quote_attribution(ctx, date, sender),
            headers = listOf(
                L10n.quote_from(ctx) to sender,
                L10n.quote_sent(ctx) to date,
                L10n.quote_to(ctx) to L10n.quote_preview_to(ctx),
                L10n.quote_subject(ctx) to L10n.quote_preview_subject(ctx),
            ),
            body = L10n.quote_preview_body(ctx),
        )
    }
}
