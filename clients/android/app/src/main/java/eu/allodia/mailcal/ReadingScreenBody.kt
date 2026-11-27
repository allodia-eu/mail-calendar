// The recipient headers and the sanitised HTML/remote-image body on the reading screen, split out
// of ReadingScreen.kt. The full HTML document (strict CSP, base styling, remote-image gating) is
// built in shared Rust (`renderMessageHtml`); this supplies the native hardening a WebView needs:
// JavaScript off, in-view navigation blocked, remote sub-resource loads intercepted unless the
// user opted in. See docs/rendering-security.md.
package eu.allodia.mailcal

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import uniffi.mailcal_bindings.MessageScheme
import uniffi.mailcal_bindings.renderMessageHtml

// The bar shown above a message that has remote images, which are blocked by default to
// avoid tracking. Tapping "Load images" opts in for this message.
@Composable
internal fun RemoteImagesBanner(onLoad: () -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .padding(start = 16.dp, end = 8.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(
            text = L10n.reading_remote_blocked(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onLoad) { Text(L10n.action_load_images(ctx)) }
    }
}

// A mutable holder the WebViewClient reads so toggling "Load images" reflects in
// shouldInterceptRequest without recreating the client.
private class RemotePolicy {
    var allowRemote = false
}

// Where the reader was when the same message was drawn again, put back once the new page is on
// screen: in another scheme (docs/reading-dark-mode.md, rule 9), where only the colours changed,
// or in a view built anew after a visit to another tab, where nothing did.
//
// Put back when the page commits, and only then. For an in-memory document `onPageFinished`
// arrives first, and the new page's first frame then scrolls it to the top, so a place spent
// there is overwritten before anyone sees it.
internal class KeptPlace {
    private var scrollY: Int? = null

    // A document is about to load. `sameMessage` is the same message drawn again, whose place is
    // worth keeping; anything else opens at its top.
    fun loading(currentScrollY: Int, sameMessage: Boolean) {
        scrollY = currentScrollY.takeIf { sameMessage && it > 0 }
    }

    // The new page is on screen: where to scroll it, once.
    fun committed(): Int? = scrollY.also { scrollY = null }
}

// What a document is built from; a change of any of them reloads it.
private data class DocumentInputs(
    val fragment: String,
    val loadRemoteImages: Boolean,
    val scheme: MessageScheme,
)

// Every WebSettings flag the reading host depends on, in one place so the JVM suite can state
// them: each is a contract cell, and every one of them is a default this happens to agree or
// disagree with, which a reader of the factory below cannot tell apart.
internal fun WebSettings.applyReadingPolicy() {
    // The native half of the rendering gates (docs/rendering-security.md).
    javaScriptEnabled = false
    allowFileAccess = false
    allowContentAccess = false
    // Fitting (docs/reading-zoom.md). `useWideViewPort` is what makes the shared document's
    // `width=device-width` reach the engine at all: left at its default of **false** the viewport
    // tag is ignored outright, so a newsletter's own mobile rules never fire and it lays out at
    // 600px on a phone. Honouring a viewport tag is safe here precisely because the sanitiser
    // drops `<meta>`, so the only one in the document is ours. `loadWithOverviewMode` then scales
    // a message too wide for the pane down until it fits, which is the case no `@media` rule can
    // reach; it does nothing without the wide viewport above.
    useWideViewPort = true
    loadWithOverviewMode = true
    // The reader's pinch, without the legacy floating +/- buttons over the message.
    builtInZoomControls = true
    displayZoomControls = false
}

// The reading host's second barrier behind the document CSP, shared with the print host: an empty
// answer for a remote http(s) sub-resource (image, font, CSS) the reader has not opted into, and
// null, meaning "load it", for everything else.
internal fun blockRemoteLoad(request: WebResourceRequest?, allowRemote: Boolean): WebResourceResponse? {
    val scheme = request?.url?.scheme?.lowercase()
    return if (!allowRemote && (scheme == "http" || scheme == "https")) {
        WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))
    } else {
        null
    }
}

// Renders the core's sanitised HTML in a hardened WebView. The full document (strict CSP,
// base styling, remote-image gating) is produced by shared Rust (`renderMessageHtml`); this
// adds the native defenses: JavaScript disabled, in-view navigation blocked (tapped links
// open in the default browser), and, as a second barrier to the document CSP, remote
// sub-resource loads intercepted unless the user opted into images.
@Composable
internal fun HtmlBody(fragment: String, loadRemoteImages: Boolean, scheme: MessageScheme) {
    val policy = remember { RemotePolicy() }
    policy.allowRemote = loadRemoteImages
    val kept = remember { KeptPlace() }
    // Where the reader is in this message, kept while another tab is up: the view is built anew on
    // the way back, and opens where they left it.
    var place by rememberSaveable(fragment) { mutableIntStateOf(0) }
    // The Compose owner view, invalidated when the WebView commits its first frame (belt-and-
    // suspenders alongside the fade below).
    val hostView = LocalView.current
    // The WebView's very first paint momentarily blanks the sibling Compose header (the "flicker"
    // on open): on that one frame the parent draws the WebView but not the Compose layers above
    // it. Two things prevent it from ever being visible: the WebView always draws into its own
    // graphics layer (`graphicsLayer` below), which decouples its paints from the parent's Compose
    // draw; and it starts transparent and fades in only once it has painted, so that first frame
    // happens while it's invisible. A spinner covers the body until then.
    var painted by remember { mutableStateOf(false) }
    val alpha by animateFloatAsState(
        targetValue = if (painted) 1f else 0f,
        animationSpec = tween(durationMillis = 150),
        label = "readingBodyFade",
    )
    Box(modifier = Modifier.fillMaxSize()) {
        AndroidView(
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer { this.alpha = alpha },
            factory = { context ->
            WebView(context).apply {
                settings.applyReadingPolicy()
                setOnScrollChangeListener { _, _, y, _, _ -> place = y }
                webViewClient = object : WebViewClient() {
                    // First frame is painted: reveal (fade in) and nudge the owner to redraw.
                    override fun onPageCommitVisible(view: WebView?, url: String?) {
                        kept.committed()?.let { view?.scrollTo(0, it) }
                        painted = true
                        hostView.invalidate()
                    }

                    // Fallback so the spinner can never get stuck if the commit-visible signal
                    // doesn't arrive for a given body (e.g. empty content).
                    override fun onPageFinished(view: WebView?, url: String?) {
                        painted = true
                    }

                    // The body is inert, we never navigate in place. A link the user tapped
                    // opens in the system default browser/handler instead; whether to open it
                    // is the shared-Rust launch policy (`shouldOpenExternalLink`) so every
                    // client is identical. Everything else is just cancelled. See
                    // docs/rendering-security.md.
                    override fun shouldOverrideUrlLoading(
                        view: WebView?,
                        request: WebResourceRequest?,
                    ): Boolean {
                        val url = request?.url
                        val context = view?.context
                        if (request?.hasGesture() == true && url != null && context != null) {
                            openExternalLink(context, url.toString())
                        }
                        return true
                    }

                    // Defence in depth atop the document CSP: hard-block remote http(s)
                    // sub-resource loads (images, fonts, CSS) unless the user opted in.
                    override fun shouldInterceptRequest(
                        view: WebView?,
                        request: WebResourceRequest?,
                    ): WebResourceResponse? = blockRemoteLoad(request, policy.allowRemote)
                }
            }
        },
            // baseURL null → no origin to resolve relative resources against; the document CSP is
            // the boundary. Reload only when the inputs change (fragment, load-images choice or
            // scheme), so unrelated recompositions don't rebuild the document (an FFI call) or
            // reload. A change of scheme alone keeps the reader's place.
            update = { webView ->
                val inputs = DocumentInputs(fragment, loadRemoteImages, scheme)
                val last = webView.tag as? DocumentInputs
                if (last != inputs) {
                    webView.tag = inputs
                    // A view built anew for a message already read returns to its place.
                    kept.loading(
                        currentScrollY = if (last == null) place else webView.scrollY,
                        sameMessage = last == null || last.copy(scheme = scheme) == inputs,
                    )
                    val document = renderMessageHtml(fragment, loadRemoteImages, scheme)
                    webView.loadDataWithBaseURL(null, document, "text/html", "utf-8", null)
                }
            },
        )
        // Show a spinner over the still-transparent WebView until it has painted, so the reading
        // area shows progress rather than a blank rectangle during the fade-in.
        if (!painted) {
            CenteredMessage { CircularProgressIndicator() }
        }
    }
}

@Composable
internal fun CenteredMessage(content: @Composable () -> Unit) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { content() }
}
