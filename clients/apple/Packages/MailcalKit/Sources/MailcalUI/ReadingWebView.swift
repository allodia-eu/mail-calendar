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

#if os(iOS)
/// Scales a message too wide for the pane down until it fits (docs/reading-zoom.md, rule 3).
///
/// WebKit does **not** do this for us. Blink has `loadWithOverviewMode`, which is what makes the
/// rule hold on Android; WebKit has no equivalent, and `shrink-to-fit` does nothing for a document
/// whose viewport names a width, so a 600px newsletter lays out at 600pt in a 402pt pane and runs
/// off the right edge. Measured on an iPhone simulator, not read off the documentation, because the
/// documentation is what suggested it was already handled.
///
/// The scale can only be known from the laid-out document, which is why this is native and
/// per-platform rather than part of the shared document: no CSS knows the content's width, and
/// measuring it from inside the message would need script (rendering-security.md, gates 1 and 2).
/// `contentSize` is the host asking its own view a question, not the message running anything.
///
/// ⚠️ **Not called from `didFinish`: there is nothing to measure yet.** Measured on an iPhone
/// simulator, `scrollView.contentSize.width` is still **0** when that delegate fires, so a fit
/// computed there divides by nothing and silently leaves the message clipped, which reads as the
/// rule not working rather than as the measurement being early. The content size arriving is the
/// signal, so `Coordinator` observes it (`watchContentSize`).
///
/// An image arriving later cannot widen the document past the fit: the base stylesheet caps every
/// image at `max-width:100%` of its container.
@MainActor
func fitReadingDocument(_ webView: WKWebView) {
    let content = webView.scrollView.contentSize.width
    let pane = webView.bounds.width
    guard content > 0, pane > 0 else { return }
    // Only ever shrink. A message narrower than the pane keeps its own size rather than being
    // blown up to fill it, which would magnify a short plain-text note to nonsense.
    let fit = min(1, pane / content)
    guard fit < 1 else { return }
    // `pageZoom`, not the scroll view's `zoomScale`. WebKit owns that scroll view: it recomputes
    // `minimumZoomScale`/`maximumZoomScale` from the viewport on its own layout pass and clamps
    // an assignment back, so the message stays clipped and nothing reports why. `pageZoom` is a
    // layout zoom WebKit applies itself, which is also the better answer: the message is laid out
    // again at the smaller scale rather than having its rendered surface resampled.
    webView.pageZoom = fit
}

#endif

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
    #else
    // The fit `fitReadingDocument` applied is the *view's*, exactly as `magnification` is, so it
    // survives the load: without this, a message opened after a wide one inherits that message's
    // scale and renders small for no reason the reader can see. Reset before the load, so the
    // measurement that follows is taken at 1:1 and the ratio it computes means what it says.
    webView.pageZoom = 1
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
        #if os(iOS)
        context.coordinator.watchContentSize(of: webView)
        #endif
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
        #if os(iOS)
        // A new message: it gets its own fit once it has a width to measure.
        coordinator.awaitingFit = true
        #endif
        let document = renderMessageHtml(html: fragment, loadRemoteImages: loadRemoteImages)
        loadReadingDocument(webView, document)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        var lastFragment: String?
        var lastLoadRemoteImages: Bool?

        #if os(iOS)
        /// Whether the document now on screen still owes us a fit. Set when one is handed over,
        /// cleared by the fit, so a content size that keeps changing (the fit itself changes it)
        /// cannot re-fit a message that has already been scaled.
        var awaitingFit = false
        private var contentSize: NSKeyValueObservation?

        /// Watches for the document acquiring a width, which is the only signal that it can be
        /// measured; see [`fitReadingDocument`] for why `didFinish` is too early. Installed once
        /// per web view, and the observation lives as long as this coordinator does.
        func watchContentSize(of webView: WKWebView) {
            guard contentSize == nil else { return }
            contentSize = webView.scrollView.observe(\.contentSize) { [weak self] scrollView, _ in
                MainActor.assumeIsolated {
                    guard let self, self.awaitingFit, scrollView.contentSize.width > 0 else {
                        return
                    }
                    self.awaitingFit = false
                    fitReadingDocument(webView)
                }
            }
        }
        #endif

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
