// The hardening every host of the shared editor bundle applies (docs/composer-security.md): the
// message composer AND the Settings signature editor. It lives here, in one place, because it is a
// security contract rather than a per-screen detail, authoring a signature is authoring mail
// content, so it gets the composer's gates, not a lighter set. Two hosts with two copies of these
// settings is two chances for one of them to drift.
//
// What it guarantees: JavaScript runs for the bundled document only, no window can be opened, no
// file or content:// URL is readable, no DOM storage persists, no mixed content loads, every
// navigation away is refused, and every http/https subresource request is answered with an empty
// body, the native barrier behind the bundle's own CSP.
package eu.allodia.mailcal

import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import java.io.ByteArrayInputStream

internal fun WebView.applyEditorSecuritySettings() {
    settings.javaScriptEnabled = true
    settings.javaScriptCanOpenWindowsAutomatically = false
    settings.setSupportMultipleWindows(false)
    settings.allowFileAccess = false
    settings.allowContentAccess = false
    settings.domStorageEnabled = false
    settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
}

// The navigation + subresource gate. Subclasses add their own `onPageFinished` (the hooks each host
// injects once the bundle has parsed) and inherit both refusals.
internal open class EditorWebViewClient : WebViewClient() {
    override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean = true

    override fun shouldInterceptRequest(
        view: WebView?,
        request: WebResourceRequest?,
    ): WebResourceResponse? {
        val scheme = request?.url?.scheme?.lowercase()
        return if (scheme == "http" || scheme == "https") {
            WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))
        } else {
            null
        }
    }
}
