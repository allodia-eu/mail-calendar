// The page the composer's and the signature editor's message is written on
// (docs/reading-dark-mode.md, "Writing in a dark app"): dark exactly when a reading view's would be
// (`messageScheme`), named to the editor before its document runs and again whenever the app's
// appearance or the dark-messages preference moves. The page, every colour moved on it and the
// toolbar's sun/moon are the core's and the bundle's; this host only names the scheme and hands
// the editor's messages on.
package eu.allodia.mailcal

import android.webkit.WebView
import androidx.compose.runtime.compositionLocalOf
import uniffi.mailcal_bindings.composerColoursAnswerScript
import uniffi.mailcal_bindings.composerPagePreloadScript
import uniffi.mailcal_bindings.composerPageScript

/**
 * The reader's dark-messages preference, and [choose], which hands the core a message the editor
 * posted and answers whether it was the toolbar's sun or moon (`composerBackgroundChoice`).
 */
internal class ComposerPageHost(val darkMessages: Boolean, val choose: (String) -> Boolean)

/**
 * The page's host for every editor under it, or null where no page is named: before the core has
 * been read, and in the JVM suite, which loads no native library. An editor without one keeps the
 * light page the bundle starts on and is never asked about colours.
 */
internal val LocalComposerPage = compositionLocalOf<ComposerPageHost?> { null }

/**
 * Leaves the WebView without a canvas of its own. Android's WebView draws white until a document
 * has painted, which in a dark app is a light first frame however early the page is named; the
 * app's own surface shows through instead, until the editor draws its page.
 */
internal fun WebView.drawNoCanvasOfItsOwn() {
    setBackgroundColor(android.graphics.Color.TRANSPARENT)
}

// What the editor reads before its document runs: the app's scheme for its chrome, and the page.
internal fun composerPreloadScript(appDark: Boolean, page: ComposerPageHost?): String {
    val chrome = "window.composerScheme = \"${if (appDark) "dark" else "light"}\";"
    return chrome + (page?.let { composerPagePreloadScript(messageScheme(appDark, it.darkMessages)) } ?: "")
}

/**
 * [html] with [preload] run ahead of the bundle's own script, so the editor's first frame is drawn
 * on the page rather than on the light one the page-finished scripts would then repaint. Android's
 * WebView has no document-start hook of its own; the bundle's CSP allows inline scripts, and
 * everything in [preload] is the core's. Unchanged when the document carries no script.
 */
internal fun composerHtmlWithPreload(html: String, preload: String): String {
    val at = html.indexOf("<script")
    if (at < 0) return html
    // No `</` inside the element, so nothing in it can close it early.
    return html.substring(0, at) + "<script>${preload.replace("</", "<\\/")}</script>\n" + html.substring(at)
}

// The app's scheme and the page, as they are now: sent once the bundle has parsed, and again from
// each host's AndroidView update block, which runs when either moves.
internal fun composerAppearanceScript(appDark: Boolean, page: ComposerPageHost?): String {
    val scheme = composerSchemeScript(appDark)
    return page?.let { "$scheme;${composerPageScript(messageScheme(appDark, it.darkMessages))}" } ?: scheme
}

/**
 * Hands a message the editor posted to the core's two readers of the page: a colours request is
 * answered with the core's script as it is, and the toolbar's sun/moon sets the preference, whose
 * SETTINGS signal names the page back to every editor and reading view.
 */
internal fun WebView.answerComposerPage(message: String, page: ComposerPageHost?) {
    page ?: return
    val answer = composerColoursAnswerScript(message)
    if (answer != null) {
        evaluateJavascript(answer, null)
        return
    }
    page.choose(message)
}
