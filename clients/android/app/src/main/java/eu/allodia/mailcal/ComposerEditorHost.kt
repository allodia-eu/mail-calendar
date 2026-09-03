// Hosting the shared editor bundle in the composer's WebView: what is injected once the page has
// parsed, how the WebView is configured, and the two seams the composer drives afterwards (the
// signature swap, the caret+keyboard focus). Split out of RichComposeScreen.kt so that file stays
// the composer *screen* and this one is the editor *host*, the same split the Apple client makes
// between RichComposerView and RichComposerEditor.
package eu.allodia.mailcal

import android.view.inputmethod.InputMethodManager
import android.webkit.WebView
import org.json.JSONArray
import org.json.JSONObject
import uniffi.mailcal_bindings.SignatureBody

// Seeds, replaces, or removes THIS message's signature region. Null removes it ("None", or an
// account with no signature); the editor keeps the user's typed text and their trimming of the
// quote either way, and only ever touches the region that is a direct child of the editor (a quoted
// original may carry its sender's signature, see docs/signatures.md).
internal fun WebView.setComposerSignature(body: SignatureBody?) {
    val seed = signatureSeedJson(body)
    val argument = if (seed == null) "null" else JSONObject.quote(seed)
    evaluateJavascript("window.setComposerSignature($argument)", null)
}

// Shows a picture at the caret. The shared editor records the inline attachment behind it and
// carries the bytes in the document, so the core can turn it into the `cid:` part the sent body
// points at; the same path a pasted screenshot takes, so a dropped and a pasted picture cannot
// behave differently.
internal fun WebView.insertComposerImage(dataUrl: String, fileName: String) {
    val payload = JSONObject().apply {
        put("data_url", dataUrl)
        put("file_name", fileName)
    }
    evaluateJavascript("window.insertComposerImage($payload)", null)
}

// The editor's localised chrome, as a JSON object literal for setComposerLabels. Built from the
// l10n catalog so it follows the app's UI language; the keys mirror setComposerLabels in editor.html.
internal fun composerLabelsJson(ctx: android.content.Context): String = JSONObject().apply {
    put("placeholder", L10n.editor_placeholder(ctx))
    put("bold", L10n.editor_bold(ctx))
    put("italic", L10n.editor_italic(ctx))
    put("underline", L10n.editor_underline(ctx))
    put("fontSize", L10n.editor_font_size(ctx))
    put("sizeNormal", L10n.editor_size_normal(ctx))
    put("sizeSmall", L10n.editor_size_small(ctx))
    put("sizeLarge", L10n.editor_size_large(ctx))
    put("sizeHuge", L10n.editor_size_huge(ctx))
    put("bulletedList", L10n.editor_bulleted_list(ctx))
    put("numberedList", L10n.editor_numbered_list(ctx))
    put("indent", L10n.editor_indent(ctx))
    put("outdent", L10n.editor_outdent(ctx))
    put("textColour", L10n.editor_text_colour(ctx))
    put("colourAutomatic", L10n.editor_colour_automatic(ctx))
    put("highlight", L10n.editor_highlight(ctx))
    put("highlightNone", L10n.editor_highlight_none(ctx))
    put("table", L10n.editor_table(ctx))
    put("insertTable", L10n.editor_insert_table(ctx))
    put("insertRowAbove", L10n.editor_insert_row_above(ctx))
    put("insertRowBelow", L10n.editor_insert_row_below(ctx))
    put("insertColumnLeft", L10n.editor_insert_column_left(ctx))
    put("insertColumnRight", L10n.editor_insert_column_right(ctx))
    put("deleteRow", L10n.editor_delete_row(ctx))
    put("deleteColumn", L10n.editor_delete_column(ctx))
    put("deleteTable", L10n.editor_delete_table(ctx))
}.toString()

// The exact JS the host injects once the editor bundle has parsed and its `window.*` hooks exist,
// in order. This list is the whole open-time contract, and it has to be *complete*: a hook called
// any earlier lands on an undefined function and fails silently, leaving the editor in its default
// state with no error anywhere.
//
// `setComposerTopInset` is the one that bites. Sending it only from a layout-time effect loses the
// race with the page parse, and the editor then keeps its 14px CSS default padding, which puts the
// entire typing area underneath the opaque header overlay, where it is untappable and anything
// typed is invisible.
//
// The signature goes LAST, after the quote: the editor decides where to place it on first insert:
// above the quoted original when there is one, so seeding it before the quote exists would put a
// reply's signature at the bottom, under the message it is replying to.
internal fun composerPageFinishedScripts(
    labelsJson: String,
    quote: String?,
    topInsetDp: Float,
    signature: String? = null,
    body: String = "",
): List<String> = buildList {
    add("window.useNativeComposerChrome()")
    if (topInsetDp > 0f) {
        add("window.setComposerTopInset($topInsetDp)")
    }
    add("window.setComposerLabels($labelsJson)")
    if (quote != null) {
        add("window.setComposerQuote(${JSONObject.quote(quote)})")
    }
    // A mail link's `body=`, seeded as text (docs/composer-security.md, Gate 12). It goes before
    // the signature and after the quote because `setPlainText` assigns the WHOLE body: anything
    // seeded before it is overwritten, so the signature would be gone and the quote would have to
    // be re-seeded. In practice a link never carries a quote, a quote only ever seeds a
    // reply/forward, but the order makes the editor's state defined rather than call-order luck.
    if (body.isNotEmpty()) {
        add("window.setPlainText(${JSONObject.quote(body)})")
    }
    if (signature != null) {
        add("window.setComposerSignature(${JSONObject.quote(signature)})")
    }
}

internal fun WebView.configureComposerWebView(
    quote: String?,
    // The plain-text body the composer opens with, a mail link's `body=`, empty otherwise.
    body: String = "",
    labelsJson: String,
    focusBody: Boolean,
    topInsetDp: () -> Float,
    // Read at page-finished time rather than captured now, for the same reason as the top inset:
    // the From account (and so the signature it resolves to) settles during composition, and the
    // factory that builds this WebView runs only once.
    signature: () -> String?,
    onScroll: (Int) -> Unit,
    // The document as it stood once the seeds were in, the "nothing written yet" baseline the
    // discard prompt compares against (the macOS/Windows rule). Null if the editor
    // never answered, which reads as "no draft to lose".
    onSeeded: (String?) -> Unit = {},
) {
    // The shared hardening (EditorWebView.kt), the same gates the Settings signature editor gets.
    applyEditorSecuritySettings()
    installEditorLinkMenu()
    // No inner scrollbar: the page scrolls as one and the native header overlay tracks this offset.
    isVerticalScrollBarEnabled = false
    setOnScrollChangeListener { _, _, scrollYNew, _, _ -> onScroll(scrollYNew) }
    webViewClient = object : EditorWebViewClient() {
        // The editor bundle has finished loading, so its window.* hooks now exist. Doing this here
        // not right after loadDataWithBaseURL, and not from a layout-time effect, is what
        // guarantees they are defined. See composerPageFinishedScripts for the contract.
        override fun onPageFinished(view: WebView?, url: String?) {
            for (script in composerPageFinishedScripts(labelsJson, quote, topInsetDp(), signature(), body)) {
                view?.evaluateJavascript(script, null)
            }
            // Snapshot the seeded document as the discard prompt's baseline. Queued AFTER the
            // seeds (the WebView runs these in order) so a reply that merely carries its quoted
            // original and a signature does not open already "dirty", and BEFORE the focus call,
            // for the same reason: moving the caret is not the user having written something.
            view?.evaluateJavascript("composerDocument()") { encoded ->
                onSeeded(decodeJsString(encoded))
            }
            if (focusBody) {
                view?.focusEditorAndShowKeyboard()
            }
        }
    }
}

// Puts the caret in the message body and brings the soft keyboard up, so a reply opens ready to
// type. Two separate things have to be true for that, which is why this is more than one call:
// the editor's contenteditable needs *DOM* focus (the editor bundle focuses it on load, but a
// seeded quote replaces the body, so re-assert it), and the WebView needs Android *view* focus:
// the DOM focus alone never raises the IME. showSoftInput is posted because it is a no-op until
// the view is attached and actually holds focus.
private fun WebView.focusEditorAndShowKeyboard() {
    isFocusableInTouchMode = true
    evaluateJavascript("window.focusComposerBody()", null)
    post {
        if (requestFocus()) {
            context.getSystemService(InputMethodManager::class.java)
                ?.showSoftInput(this, InputMethodManager.SHOW_IMPLICIT)
        }
    }
}

internal fun loadComposerAsset(ctx: android.content.Context): String = try {
    ctx.assets.open("editor.html").bufferedReader(Charsets.UTF_8).use { it.readText() }
} catch (_: Exception) {
    "<!doctype html><html><body><script>window.composerDocument=function(){return JSON.stringify({blocks:[],attachments:[]});};</script></body></html>"
}

internal fun decodeJsString(encoded: String?): String? {
    if (encoded.isNullOrBlank() || encoded == "null") {
        return null
    }
    return try {
        JSONArray("[$encoded]").getString(0)
    } catch (_: Exception) {
        null
    }
}
