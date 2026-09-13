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

#if os(iOS)
/// A message host that says when it has been laid out.
///
/// The reading header is a subview of this view's scroll view, sized against its width, and that
/// width is not knowable from `updateUIView`: SwiftUI gives a representable's view its frame
/// **after** that call, so the first update sees zero, and a later layout on its own, a rotation or
/// an iPad column being dragged, produces no update at all. The layout is the signal, so the view
/// reports it.
final class ReadingBodyWebView: WKWebView {
    /// Called after every layout pass, with the view's frame settled.
    var onLayout: (() -> Void)?

    override func layoutSubviews() {
        super.layoutSubviews()
        onLayout?()
    }
}

/// The reading pane's web view: the host above on iOS and iPadOS, `WKWebView` itself on macOS,
/// which has no scroll view for a header to live in.
typealias ReadingBodyView = ReadingBodyWebView
#else
typealias ReadingBodyView = WKWebView
#endif

/// Builds the hardened host for a message body.
///
/// Separate from `SanitizedHTMLView.makeNSView` because a SwiftUI `Context` cannot be constructed,
/// so the representable's own entry points are unreachable from the suite; the policy it applies is
/// exactly what a test needs to state, and here it can.
@MainActor
func makeReadingWebView() -> ReadingBodyView {
    let config = WKWebViewConfiguration()
    config.defaultWebpagePreferences.allowsContentJavaScript = false
    let webView = ReadingBodyView(frame: .zero, configuration: config)
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
/// one's. On macOS `magnification` is the *view's* rather than the page's, so unlike the iOS page
/// scale it survives the load and would otherwise carry over to whatever is opened next. On
/// iOS/iPadOS there is nothing to reset: the only scale is the page's own, and WebKit drops that
/// on load.
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
    #if os(iOS)
    /// The reading header, which rides this message's own scroll (ReadingView.Scroll.swift). It is
    /// handed over as a view rather than drawn above this one, so that it can be **hosted inside
    /// the web view's scroll view**: the one place from which WebKit's own pan gesture reaches it.
    let header: AnyView
    #endif

    func makeCoordinator() -> Coordinator { Coordinator() }

    #if os(macOS)
    func makeNSView(context: Context) -> ReadingBodyView { makeWebView(context) }
    func updateNSView(_ webView: ReadingBodyView, context: Context) {
        updateWebView(webView, context)
    }
    #else
    func makeUIView(context: Context) -> ReadingBodyView { makeWebView(context) }
    func updateUIView(_ webView: ReadingBodyView, context: Context) {
        updateWebView(webView, context)
    }
    #endif

    private func makeWebView(_ context: Context) -> ReadingBodyView {
        let webView = makeReadingWebView()
        webView.navigationDelegate = context.coordinator
        #if os(iOS)
        context.coordinator.mountHeader(in: webView)
        #endif
        return webView
    }

    private func updateWebView(_ webView: ReadingBodyView, _ context: Context) {
        let coordinator = context.coordinator
        #if os(iOS)
        // Every update, before the guard below: the header grows as the snapshot fills it in, and
        // that arrives on updates which change neither of the two inputs the guard is about.
        coordinator.show(header, in: webView)
        #endif
        // Skip entirely when the inputs are unchanged, so unrelated SwiftUI updates don't
        // re-run the (FFI) document build or reload the page, only the fragment or the
        // load-images choice changing matters.
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

        #if os(iOS)
        /// The reading header, living in the web view's scroll view above the message.
        ///
        /// A subview of that scroll view rather than a SwiftUI view drawn over it, because the
        /// scroll view's pan gesture only reaches what is inside it. Drawn on top instead, the
        /// header takes every drag that lands on it, and a message whose header fills the screen,
        /// an invitation card or twenty attachments, cannot be scrolled at all.
        ///
        /// It sits at a negative `y`, which `contentInset.top` then makes the resting position, so
        /// the message starts below it and both move as the reader scrolls: the table-header
        /// pattern `UITableView` has always used, and no offset arithmetic of our own.
        private let host = UIHostingController(rootView: AnyView(EmptyView()))
        /// The header as the reading view last built it, so a layout pass can put it back without
        /// SwiftUI having to hand it over again.
        private var header = AnyView(EmptyView())

        func mountHeader(in webView: ReadingBodyWebView) {
            host.view.backgroundColor = .systemBackground
            webView.scrollView.addSubview(host.view)
            // The layout, not `updateUIView`, is when this view's width is knowable; see
            // [`ReadingBodyWebView`]. Both captures are weak: the web view owns this closure.
            webView.onLayout = { [weak self, weak webView] in
                guard let self, let webView else { return }
                self.layOutHeader(in: webView)
            }
        }

        /// Takes the header the reading view has just built, and puts it on screen.
        func show(_ header: AnyView, in webView: ReadingBodyWebView) {
            self.header = header
            layOutHeader(in: webView)
        }

        /// Puts the header back at the pane's current width, and gives the message the room under
        /// it. Idempotent, so it is safe on every update and every layout pass.
        ///
        /// ⚠️ **The width is the scroll view's, not the header's own idea of one.** Asked to size
        /// itself against an unbounded width a hosting controller answers with the width its
        /// content would like, which for a subject line is the whole subject on one line, and the
        /// header then lays out wider than the pane it is in.
        private func layOutHeader(in webView: ReadingBodyWebView) {
            let scrollView = webView.scrollView
            let width = scrollView.bounds.width
            guard width > 0 else { return }
            host.rootView = header
            let height = host.sizeThatFits(
                in: CGSize(width: width, height: .greatestFiniteMagnitude)
            ).height
            host.view.frame = CGRect(x: 0, y: -height, width: width, height: height)
            guard scrollView.contentInset.top != height else { return }
            // What the system adds on its own (the safe area), which the new resting offset has to
            // keep. ⚠️ Read **before** the assignment and carried across it: `adjustedContentInset`
            // is recomputed on the next layout pass, so reading it straight afterwards still
            // answers the old total, and an offset set from that leaves the message resting with
            // the header already scrolled off the top.
            let system = scrollView.adjustedContentInset.top - scrollView.contentInset.top
            let wasAtTop = scrollView.contentOffset.y <= -scrollView.adjustedContentInset.top
            scrollView.contentInset.top = height
            scrollView.verticalScrollIndicatorInsets.top = height
            // A reader who has not scrolled yet is still looking at the top of the header after it
            // grew; one who has scrolled keeps their place. UIKit moves the offset with the inset
            // only while the scroll view is settling, not for a change this late.
            if wasAtTop {
                scrollView.contentOffset.y = -(height + system)
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
