// Builds the seed for the composer's quoted original on reply/forward. The quoted body is the
// reading view's already-sanitised HTML (and plain text) for the open message; the attribution
// is localised here, the Rust core carries no runtime localisation, so, like date display, the
// client formats it (L10n + the device-formatted date already on `OpenedMessage`). The shape
// matches the Rust composer's `Block::Quote` so it round-trips through the shared composer; the
// core re-sanitises the body on submit (docs/composer-security.md, Gate 10). The Android twin of
// macOS's QuoteSeed.swift.
package eu.allodia.mailcal

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.ReadingSnapshot

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
    // The seed JSON for `window.setComposerQuote`, or `null` when there is nothing to quote yet
    // (the body hasn't loaded for this message). `isForward` swaps the one-line attribution for a
    // "Forwarded message" marker; the header block is the same either way.
    // `initialText` pre-fills the paragraph above the quote; only showcase mode passes it, and the
    // editor assigns it as text, never markup (docs/composer-security.md, Gate 11).
    fun seedJson(
        ctx: Context,
        style: QuoteStyleKind,
        message: OpenedMessage,
        reading: ReadingSnapshot?,
        isForward: Boolean,
        activeZoneId: String?,
        use24Hour: Boolean,
        initialText: String? = null,
    ): String? {
        val snapshot = reading?.takeIf { it.key == message.key } ?: return null
        val bodyHtml = snapshot.html ?: ""
        val bodyPlain = snapshot.plain ?: ""
        if (bodyHtml.isEmpty() && bodyPlain.isEmpty()) {
            return null
        }

        // The reader of this quote is the *recipient*, so the date is localised exactly as the
        // reading header is (docs/timestamps.md). The core emits a UTC instant; sending it raw
        // would put `2026-08-31T05:01:00Z` in their mailbox.
        val sent = localDateTime(message.date, activeZoneId, use24Hour)
        val line = if (isForward) {
            L10n.quote_forwarded(ctx)
        } else {
            L10n.quote_attribution(ctx, sent, message.from)
        }

        val headers = JSONArray()
        headers.put(header(L10n.quote_from(ctx), message.from))
        headers.put(header(L10n.quote_sent(ctx), sent))
        if (snapshot.to.isNotEmpty()) {
            headers.put(header(L10n.quote_to(ctx), snapshot.to))
        }
        if (snapshot.cc.isNotEmpty()) {
            headers.put(header(L10n.quote_cc(ctx), snapshot.cc))
        }
        headers.put(header(L10n.quote_subject(ctx), message.subject))

        val attribution = JSONObject().put("line", line).put("headers", headers)
        return JSONObject()
            .put("style", token(style))
            .put("attribution", attribution)
            .put("body_html", bodyHtml)
            .put("body_plain", bodyPlain)
            .apply { if (!initialText.isNullOrEmpty()) put("initial_text", initialText) }
            .toString()
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
    // `seedJson` uses above, so what settings shows is what a real reply produces.
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

    private fun header(label: String, value: String): JSONObject =
        JSONObject().put("label", label).put("value", value)
}
