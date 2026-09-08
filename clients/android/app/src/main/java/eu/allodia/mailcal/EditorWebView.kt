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

import android.content.ClipData
import android.content.ClipboardManager
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.PopupMenu
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

// Long-pressing a link offers its address.
//
// Selected text already gets Cut/Copy/Paste from the system's own selection bar, so that half needs
// nothing; a link gets nothing by default, and a link inside a quoted original cannot be opened by
// tapping it either (navigation is blocked), so without this it is text the user can see and not
// use. The same item every other client offers (docs/composer-security.md, Gate 14).
//
// Returning false for anything that is not a link leaves ordinary text selection alone.
internal fun WebView.installEditorLinkMenu() {
    setOnLongClickListener { view ->
        val result = (view as WebView).hitTestResult
        val url = result.extra
        val isLink = result.type == WebView.HitTestResult.SRC_ANCHOR_TYPE ||
            result.type == WebView.HitTestResult.SRC_IMAGE_ANCHOR_TYPE
        if (!isLink || url.isNullOrBlank()) {
            return@setOnLongClickListener false
        }
        PopupMenu(context, view).apply {
            menu.add(L10n.action_copy_link(context)).setOnMenuItemClickListener {
                context.getSystemService(ClipboardManager::class.java)
                    ?.setPrimaryClip(ClipData.newPlainText(null, url))
                true
            }
        }.show()
        true
    }
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
