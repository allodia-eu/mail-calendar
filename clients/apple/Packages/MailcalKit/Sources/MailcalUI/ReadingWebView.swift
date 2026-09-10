// The reading pane's hardened WKWebView host, one file for macOS, iOS and iPadOS. Split out of
// ReadingView.swift to keep that file under the 500-line limit, and because these gates are worth
// reading as one thing.
//
// THE GATES HERE ARE A CROSS-PLATFORM CONTRACT, see docs/rendering-security.md, and how the body
// is sized inside them is a second one, docs/reading-zoom.md. A gate added or raised on one
// platform must be applied to all of them, and recorded there, in the same change.
//
// The full HTML document (strict CSP, base styling, remote-image gating) is built in shared Rust
// (`renderMessageHtml`), so every client behaves identically. What is unavoidably native lives
// here: scripting off, in-view navigation blocked (a clicked link goes to the default browser
// instead), and the reader's zoom.

#if os(macOS)
import AppKit
#else
import UIKit
#endif
import MailcalBindings
import SwiftUI
import WebKit

/// Builds the hardened host for a message body.
///
/// Separate from `SanitizedHTMLView.makeNSView` because a SwiftUI `Context` cannot be constructed,
/// so the representable's own entry points are unreachable from the suite; the policy it applies is
/// exactly what a test needs to state, and here it can.
@MainActor
func makeReadingWebView() -> WKWebView {
    let config = WKWebViewConfiguration()
    config.defaultWebpagePreferences.allowsContentJavaScript = false
    let webView = WKWebView(frame: .zero, configuration: config)
    #if os(macOS)
    // The trackpad pinch (docs/reading-zoom.md). On iOS/iPadOS the pinch is the scroll view's own
    // and needs nothing here: it follows the shared document's viewport, which deliberately names
    // no `user-scalable` and no `maximum-scale`.
    webView.allowsMagnification = true
    #endif
    return webView
}

/// Loads one message's document, at that message's own scale.
///
/// The reset and the load are one step on purpose: a zoom belongs to the message it was made on
/// (docs/reading-zoom.md), so every path that puts a new document on screen has to drop the last
/// one's. On macOS `magnification` is the *view's* rather than the page's, so unlike the iOS scroll
/// view's scale it survives the load and would otherwise carry over to whatever is opened next.
@MainActor
func loadReadingDocument(_ webView: WKWebView, _ document: String) {
    #if os(macOS)
    webView.magnification = 1
    #endif
    // baseURL nil → no origin to resolve remote/relative resources against; combined with the
    // document's CSP this guarantees no network access beyond opted-in images.
    webView.loadHTMLString(document, baseURL: nil)
}

/// Renders the core's sanitised HTML in a hardened WKWebView. The full document (strict CSP,
/// base styling, remote-image gating) is produced by shared Rust (`renderMessageHtml`); this
/// adds the native defenses: JavaScript disabled and in-view navigation blocked (a clicked
/// link opens in the default browser rather than loading inside the inert document).
struct SanitizedHTMLView: PlatformViewRepresentable {
    let fragment: String
    let loadRemoteImages: Bool

    func makeCoordinator() -> Coordinator { Coordinator() }

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { makeWebView(context) }
    func updateNSView(_ webView: WKWebView, context: Context) { updateWebView(webView, context) }
    #else
    func makeUIView(context: Context) -> WKWebView { makeWebView(context) }
    func updateUIView(_ webView: WKWebView, context: Context) { updateWebView(webView, context) }
    #endif

    private func makeWebView(_ context: Context) -> WKWebView {
        let webView = makeReadingWebView()
        webView.navigationDelegate = context.coordinator
        return webView
    }

    private func updateWebView(_ webView: WKWebView, _ context: Context) {
        // Skip entirely when the inputs are unchanged, so unrelated SwiftUI updates don't
        // re-run the (FFI) document build or reload the page, only the fragment or the
        // load-images choice changing matters.
        let coordinator = context.coordinator
        guard coordinator.lastFragment != fragment
            || coordinator.lastLoadRemoteImages != loadRemoteImages
        else { return }
        coordinator.lastFragment = fragment
        coordinator.lastLoadRemoteImages = loadRemoteImages
        let document = renderMessageHtml(html: fragment, loadRemoteImages: loadRemoteImages)
        loadReadingDocument(webView, document)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var lastFragment: String?
        var lastLoadRemoteImages: Bool?

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
        ) {
            // Allow only the initial in-document load (loadHTMLString → `.other`, an
            // about:/empty URL); the body itself is inert, we never navigate it in place.
            let url = navigationAction.request.url
            if navigationAction.navigationType == .other,
                url == nil || url?.scheme == "about" {
                decisionHandler(.allow)
                return
            }
            // A link the user clicked opens in their default browser/handler instead; the
            // in-view navigation is still cancelled, so the document stays inert. The
            // allow-or-ignore decision is the shared-Rust launch policy (`shouldOpenExternalLink`)
            // so every client is identical, see docs/rendering-security.md.
            if navigationAction.navigationType == .linkActivated,
                let url, shouldOpenExternalLink(url: url.absoluteString) {
                #if os(macOS)
                NSWorkspace.shared.open(url)
                #else
                UIApplication.shared.open(url)
                #endif
            }
            decisionHandler(.cancel)
        }
    }
}
