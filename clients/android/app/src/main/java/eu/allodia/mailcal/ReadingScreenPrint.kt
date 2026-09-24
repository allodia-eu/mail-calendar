// Printing the open message (docs/reading-actions.md, "Printing a message"). The page is built in
// shared Rust; what is native is the WebView it is laid out in, which carries the reading view's
// gates, and the system print dialog.
package eu.allodia.mailcal

import android.content.Context
import android.print.PrintAttributes
import android.print.PrintManager
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import uniffi.mailcal_bindings.PrintHeaderLine
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.renderMessagePrintHtml

/**
 * The header lines a printout carries: the ones the reading screen draws, under the same labels,
 * so a printout says what the screen said. Empty values are dropped by the core.
 */
internal fun messagePrintLines(
    ctx: Context,
    message: OpenedMessage,
    body: ReadingSnapshot,
): List<PrintHeaderLine> = listOf(
    PrintHeaderLine(L10n.compose_from(ctx), body.from.ifEmpty { message.from }),
    PrintHeaderLine(L10n.compose_to(ctx), body.to),
    PrintHeaderLine(L10n.compose_cc(ctx), body.cc),
    PrintHeaderLine(L10n.compose_bcc(ctx), body.bcc),
    PrintHeaderLine(L10n.quote_sent(ctx), message.date),
)

/** Whether `body` is something to print: an open that finished and fetched a body. */
internal fun isPrintable(body: ReadingSnapshot?): Boolean =
    body != null && !body.pending && !body.loadError

// The WebView of the print in flight, held until the job exists: the framework's adapter only
// borrows it, and the print framework asks that it not be collected before then.
private var printing: WebView? = null

/** Prints the open message: `body` is its snapshot, `loadRemoteImages` the reader's choice for it. */
internal fun printOpenMessage(
    ctx: Context,
    message: OpenedMessage,
    body: ReadingSnapshot,
    loadRemoteImages: Boolean,
) {
    val subject = message.subject.ifEmpty { L10n.mail_no_subject(ctx) }
    val document = renderMessagePrintHtml(
        subject,
        messagePrintLines(ctx, message, body),
        body.html,
        body.plain,
        loadRemoteImages,
    )
    printMessage(ctx, document, subject, loadRemoteImages)
}

/**
 * Lays `document` out in a WebView nobody sees and hands it to the system print dialog.
 *
 * The WebView takes the reading host's settings and its remote-load block, and refuses every
 * navigation, so the page is exactly as inert as the one on screen. `ctx` must be an Activity's:
 * the print dialog is started from it.
 */
private fun printMessage(ctx: Context, document: String, jobName: String, loadRemoteImages: Boolean) {
    val webView = WebView(ctx)
    printing = webView
    webView.settings.applyReadingPolicy()
    webView.webViewClient = object : WebViewClient() {
        private var started = false

        override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean = true

        override fun shouldInterceptRequest(
            view: WebView?,
            request: WebResourceRequest?,
        ): WebResourceResponse? = blockRemoteLoad(request, loadRemoteImages)

        override fun onPageFinished(view: WebView, url: String?) {
            if (started) return
            started = true
            val manager = ctx.getSystemService(PrintManager::class.java)
            manager.print(
                jobName,
                view.createPrintDocumentAdapter(jobName),
                PrintAttributes.Builder().build(),
            )
            printing = null
        }
    }
    webView.loadDataWithBaseURL(null, document, "text/html", "utf-8", null)
}
