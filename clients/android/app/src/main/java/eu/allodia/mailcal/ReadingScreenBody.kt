// The recipient headers and the sanitised HTML/remote-image body on the reading screen, split out
// of ReadingScreen.kt. The full HTML document (strict CSP, base styling, remote-image gating) is
// built in shared Rust (`renderMessageHtml`); this supplies the native hardening a WebView needs:
// JavaScript off, in-view navigation blocked, remote sub-resource loads intercepted unless the
// user opted in. See docs/rendering-security.md.
package eu.allodia.mailcal

import android.content.ActivityNotFoundException
import android.content.Intent
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import uniffi.mailcal_bindings.renderMessageHtml
import uniffi.mailcal_bindings.shouldOpenExternalLink

// The recipient headers (To / Cc / Bcc) shown below the subject/sender. Each row appears only
// when non-empty; Bcc is present only on the user's own Sent/Drafts copies.
@Composable
internal fun RecipientHeader(to: String, cc: String, bcc: String) {
    val ctx = LocalContext.current
    if (to.isEmpty() && cc.isEmpty() && bcc.isEmpty()) {
        return
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 16.dp, end = 16.dp, top = 2.dp, bottom = 2.dp),
    ) {
        RecipientRow(L10n.compose_to(ctx), to)
        RecipientRow(L10n.compose_cc(ctx), cc)
        RecipientRow(L10n.compose_bcc(ctx), bcc)
    }
}

@Composable
private fun RecipientRow(label: String, value: String) {
    if (value.isEmpty()) {
        return
    }
    Text(
        text = "$label: $value",
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        maxLines = 2,
        overflow = TextOverflow.Ellipsis,
    )
}

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

// Renders the core's sanitised HTML in a hardened WebView. The full document (strict CSP,
// base styling, remote-image gating) is produced by shared Rust (`renderMessageHtml`); this
// adds the native defenses: JavaScript disabled, in-view navigation blocked (tapped links
// open in the default browser), and, as a second barrier to the document CSP, remote
// sub-resource loads intercepted unless the user opted into images.
@Composable
internal fun HtmlBody(fragment: String, loadRemoteImages: Boolean) {
    val policy = remember { RemotePolicy() }
    policy.allowRemote = loadRemoteImages
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
                settings.javaScriptEnabled = false
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                webViewClient = object : WebViewClient() {
                    // First frame is painted: reveal (fade in) and nudge the owner to redraw.
                    override fun onPageCommitVisible(view: WebView?, url: String?) {
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
                        if (request?.hasGesture() == true && url != null &&
                            shouldOpenExternalLink(url.toString())
                        ) {
                            try {
                                view?.context?.startActivity(
                                    Intent(Intent.ACTION_VIEW, url)
                                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                                )
                            } catch (_: ActivityNotFoundException) {
                                // No app handles this scheme, ignore rather than crash.
                            }
                        }
                        return true
                    }

                    // Defence in depth atop the document CSP: hard-block remote http(s)
                    // sub-resource loads (images, fonts, CSS) unless the user opted in.
                    override fun shouldInterceptRequest(
                        view: WebView?,
                        request: WebResourceRequest?,
                    ): WebResourceResponse? {
                        val scheme = request?.url?.scheme?.lowercase()
                        return if (!policy.allowRemote && (scheme == "http" || scheme == "https")) {
                            WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))
                        } else {
                            null
                        }
                    }
                }
            }
        },
            // baseURL null → no origin to resolve relative resources against; the document CSP is
            // the boundary. Reload only when the inputs change (fragment or load-images choice),
            // so unrelated recompositions don't rebuild the document (an FFI call) or reload.
            update = { webView ->
                val inputs = fragment to loadRemoteImages
                if (webView.tag != inputs) {
                    webView.tag = inputs
                    val document = renderMessageHtml(fragment, loadRemoteImages)
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
