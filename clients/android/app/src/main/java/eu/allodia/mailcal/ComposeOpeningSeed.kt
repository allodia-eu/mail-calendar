// What every composer opens with, as plain rules over the core's `ComposeOpening`: which kind it
// is, what the editor is seeded with, the quote's attribution and the account From opens on. Kept
// apart from the composer so the JVM suite can hold them without composing a WebView; the
// localised words come from QuoteSeed.kt.
package eu.allodia.mailcal

import android.content.Context
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposeKind
import uniffi.mailcal_bindings.ComposeOpening
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind

/**
 * A quote's attribution in both styles, in the user's language: the one-line "On … wrote:" and the
 * labelled header block.
 */
internal data class QuoteWords(val line: String, val headers: List<Pair<String, String>>)

/**
 * One open composer: what the core opened it with, on the composition it joined, and the little
 * the host adds to it. Every route into a composer builds one of these through [composeContext],
 * so no route can open a composer holding less than another (`docs/drafts.md`).
 */
internal data class ComposeContext(
    val opening: ComposeOpening,
    /** The account id the From dropdown opens on (`sendAccountFor`). */
    val initialFrom: String?,
    /** The style a fresh quote is seeded in: the app default. */
    val quoteStyle: QuoteStyleKind = QuoteStyleKind.INDENTED,
    /** Whether the user opted into choosing the quote style per message. */
    val quoteStylePerMessage: Boolean = false,
    /** The `setComposerQuote` seed for a fresh reply or forward, or null. */
    val quote: String? = null,
    /** The `completeComposerQuoteAttribution` argument for a reopened quote, or null. */
    val quoteAttribution: String? = null,
    /** Files the host adds to what the core staged: a share's. */
    val extraAttachments: List<ComposerFileAttachment> = emptyList(),
) {
    val mode: RichComposeMode get() = ComposeOpeningSeed.modeOf(opening.kind)

    /** A stored message's HTML, which the editor reads back into its document; empty otherwise. */
    val storedHtml: String get() = opening.bodyHtml

    /**
     * The text the editor opens with: a stored message's own text beside its HTML, else what a
     * prefill brought when there is no quote to carry it.
     */
    val initialBody: String
        get() = if (opening.bodyHtml.isNotEmpty()) {
            opening.bodyText
        } else {
            ComposeOpeningSeed.plainBody(opening, quote)
        }

    /** Every file the composer opens holding: what the core staged, and a share's. */
    val attachments: List<ComposerFileAttachment> get() = opening.attachments + extraAttachments

    /**
     * The file count the written-work check measures against. The core's files are still in the
     * mailbox, a forward's originals or a stored draft's own, so closing over them loses nothing;
     * a share's the user chose and would have to share again, so those count from the start.
     */
    val baselineAttachments: Int get() = opening.attachments.size

    val showsStylePicker: Boolean
        get() = ComposerQuote.showsStylePicker(opening.quote != null, quoteStylePerMessage)

    val opensInBody: Boolean get() = composerOpensInBody(mode, opening.to)
}

/**
 * The composer's context for what the core opened it with. The quote's attribution is worded
 * here, with the original's date as the reading header shows it (`dateText`): for a fresh reply or
 * forward it seeds the quote, and for a reopened one it completes the style the stored quote does
 * not carry.
 */
internal fun composeContext(
    ctx: Context,
    opening: ComposeOpening,
    initialFrom: String?,
    quoteSettings: QuoteSettings,
    dateText: (String) -> String,
    extraAttachments: List<ComposerFileAttachment> = emptyList(),
): ComposeContext {
    val words = opening.quote?.let {
        ComposerQuote.words(ctx, it, dateText(it.date), opening.kind == ComposeKind.FORWARD)
    }
    return ComposeContext(
        opening = opening,
        initialFrom = initialFrom,
        quoteStyle = quoteSettings.style,
        quoteStylePerMessage = quoteSettings.perMessage,
        quote = ComposeOpeningSeed.quoteSeed(opening, words, quoteSettings.style, opening.bodyText),
        quoteAttribution = ComposeOpeningSeed.attributionCompletion(opening, words),
        extraAttachments = extraAttachments,
    )
}

/**
 * The account the From dropdown opens on: `preferred` (the opening's account), else the app-level
 * default send account, else the first configured one. A stored default naming a removed account
 * degrades to the first rather than to nothing, which is the core's own resolution order.
 */
internal fun sendAccountFor(
    preferred: String?,
    defaultSendAccount: String?,
    accounts: List<AccountRow>,
): String? = listOfNotNull(preferred, defaultSendAccount)
    .firstNotNullOfOrNull { id -> accounts.firstOrNull { it.id == id } }
    ?.id
    ?: accounts.firstOrNull()?.id

internal object ComposeOpeningSeed {
    fun modeOf(kind: ComposeKind): RichComposeMode = when (kind) {
        ComposeKind.NEW -> RichComposeMode.New
        ComposeKind.REPLY -> RichComposeMode.Reply
        ComposeKind.REPLY_ALL -> RichComposeMode.ReplyAll
        ComposeKind.FORWARD -> RichComposeMode.Forward
    }

    /**
     * The signatures a composer may seed, swap and offer: none at all for one the core says seeds
     * no signature, because a stored message already carries its own and a second would go out
     * under it.
     */
    fun signing(signatures: ComposerSignatures?, opening: ComposeOpening): ComposerSignatures? =
        signatures?.takeIf { opening.seedsSignature }

    /**
     * The seed for `setComposerQuote`: a fresh reply's or forward's quoted original, with
     * `initialText` above it (what a prefill brought), or null when the composer does not open
     * that way. A stored message opens through its own HTML instead, quote included.
     */
    fun quoteSeed(
        opening: ComposeOpening,
        words: QuoteWords?,
        style: QuoteStyleKind,
        initialText: String?,
    ): String? {
        val quote = opening.quote
        if (opening.bodyHtml.isNotEmpty() || quote == null || words == null) {
            return null
        }
        if (quote.bodyHtml.isEmpty() && quote.bodyPlain.isEmpty()) {
            return null
        }
        return JSONObject()
            .put("style", ComposerQuote.token(style))
            .put("attribution", attribution(words))
            .put("body_html", quote.bodyHtml)
            .put("body_plain", quote.bodyPlain)
            .apply { if (!initialText.isNullOrEmpty()) put("initial_text", initialText) }
            .toString()
    }

    /**
     * The argument for `completeComposerQuoteAttribution`: a reopened message's quote is missing
     * the attribution of the style it was not saved in, and the original says what it is. Null
     * for anything else.
     */
    fun attributionCompletion(opening: ComposeOpening, words: QuoteWords?): String? =
        if (opening.bodyHtml.isNotEmpty() && opening.quote != null && words != null) {
            attribution(words).toString()
        } else {
            null
        }

    /**
     * The text the editor opens with when it is seeded neither with a stored body nor with a
     * quote: a mail link's or a share's. Empty otherwise.
     */
    fun plainBody(opening: ComposeOpening, quoteSeed: String?): String =
        if (opening.bodyHtml.isEmpty() && quoteSeed == null) opening.bodyText else ""

    /**
     * The style of the quote a composer's document holds, or null when it holds none: what a
     * reopened composer's style picker opens on.
     */
    fun quoteStyleOf(documentJson: String?): QuoteStyleKind? {
        if (documentJson.isNullOrEmpty()) {
            return null
        }
        return try {
            val blocks = JSONObject(documentJson).getJSONArray("blocks")
            (0 until blocks.length())
                .mapNotNull { blocks.optJSONObject(it)?.optJSONObject("Quote") }
                .firstOrNull { it.has("style") }
                ?.let { styleOf(it.getString("style")) }
        } catch (_: JSONException) {
            null
        }
    }

    private fun styleOf(token: String): QuoteStyleKind? = when (token) {
        ComposerQuote.token(QuoteStyleKind.LINE_AND_HEADER) -> QuoteStyleKind.LINE_AND_HEADER
        ComposerQuote.token(QuoteStyleKind.INDENTED) -> QuoteStyleKind.INDENTED
        else -> null
    }

    private fun attribution(words: QuoteWords): JSONObject {
        val headers = JSONArray()
        words.headers.forEach { (label, value) ->
            headers.put(JSONObject().put("label", label).put("value", value))
        }
        return JSONObject().put("line", words.line).put("headers", headers)
    }
}

/**
 * The argument `setComposerBody` takes for a reopened message: its HTML, which the editor reads
 * back into its document, and its text for when it has none, as one JSON object.
 */
internal fun reopenedBodySeed(html: String, text: String): String =
    JSONObject().put("html", html).put("text", text).toString()
