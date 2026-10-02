// The editor's request channel (docs/composer-security.md, Gate 2): one injected object with one
// method, on the composer's and the signature editor's WebViews. A message is parsed by the core
// (`parseComposerHostRequest`), which knows the whole vocabulary, so anything handed on from here
// is a request the product has a type for; an answer goes back through the script the core builds.
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.ComposerHostRequest
import uniffi.mailcal_bindings.ComposerLinkAnswer
import uniffi.mailcal_bindings.composerHostRequestsScript
import uniffi.mailcal_bindings.composerLinkAnswerScript
import uniffi.mailcal_bindings.parseComposerHostRequest

// The link dialog's starting point.
internal data class LinkDialogRequest(
    val id: ULong,
    // The selected words, or the existing link's.
    val text: String,
    // The existing link's target, empty when there is none.
    val address: String,
    // Whether the selection touches a link, so the dialog offers to remove it.
    val removable: Boolean,
)

// The object the page sees as `window.composerHost`. `@JavascriptInterface` methods run on the
// WebView's own bridge thread, so every request is posted to the main thread before it reaches
// Compose state. The parser and the poster are parameters only so the JVM suite can run this
// without the native library or a looper.
internal class ComposerHostChannel(
    private val onLink: (LinkDialogRequest) -> Unit,
    private val parse: (String) -> ComposerHostRequest? = ::parseComposerHostRequest,
    private val post: (Runnable) -> Unit = Handler(Looper.getMainLooper())::post,
) {
    @JavascriptInterface
    fun postMessage(message: String) {
        when (val request = parse(message) ?: return) {
            is ComposerHostRequest.Link -> {
                val link = LinkDialogRequest(request.id, request.text, request.address, request.removable)
                post { onLink(link) }
            }
        }
    }
}

// The link dialog a WebView is waiting on, if any, and the channel that fills it in.
internal class ComposerLinkHost {
    var request by mutableStateOf<LinkDialogRequest?>(null)
    val channel = ComposerHostChannel(onLink = { request = it })
}

@Composable
internal fun rememberComposerLinkHost(): ComposerLinkHost = remember { ComposerLinkHost() }

// The core's `COMPOSER_HOST_CHANNEL`, the name the editor bundle looks for. Spelled here rather
// than asked for over the FFI because the composer is built when its WebView is, and the JVM suite
// builds the real composer without the native library.
internal const val COMPOSER_HOST_CHANNEL = "composerHost"

// Injects the channel. Before the page loads: an injected object appears only on the next load.
internal fun WebView.installComposerHost(host: ComposerLinkHost) {
    addJavascriptInterface(host.channel, COMPOSER_HOST_CHANNEL)
}

// Tells the editor which requests this host answers, once the page has loaded.
internal fun WebView.announceComposerHost() {
    evaluateJavascript(composerHostRequestsScript(), null)
}

// The link dialog, drawn inside the composer's own Dialog so its window stacks above it and back
// reaches it first. Answering closes it and puts the keyboard back in the message.
@Composable
internal fun ComposerLinkDialogHost(host: ComposerLinkHost, webView: WebView?) {
    val request = host.request ?: return
    LinkDialog(request) { answer: ComposerLinkAnswer ->
        host.request = null
        webView?.requestFocus()
        webView?.evaluateJavascript(composerLinkAnswerScript(request.id, answer), null)
    }
}
