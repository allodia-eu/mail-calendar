// The reading pane's body area: which of an open's states is on screen (the gap before the body
// lands, the spinner, a load error, the sanitised HTML, a plain-text note), the page they are all
// drawn on, and the remote-images banner above them. Split out of ReadingView.swift to keep it
// under the 500-line limit.
//
// The security gates here are a CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any
// gate added/raised on one platform must be applied to all of them (and recorded there).

import MailcalBindings
import SwiftUI

extension ReadingView {
    /// Whether the pane offers to load this message's blocked remote images: only over a body
    /// that has some, and only until the user says yes. Not `private`, see [`loadRemoteImages`].
    var remoteImagesOffered: Bool {
        guard let body = bodySnapshot, !body.pending, !body.loadError, !loadRemoteImages,
            let html = body.html, !html.isEmpty
        else { return false }
        return body.hasRemoteImages
    }

    /// The body area, on the page the core says a message is drawn on.
    ///
    /// The page is the same for every state of an open (the gap before the body lands, the
    /// spinner, a plain-text body, a load error) so a body arriving changes what is written on
    /// the page and never the page itself. Leaving the gap `Color.clear` punched a hole in it:
    /// against a dark appearance the body area went white, black, white on every message opened,
    /// which reads as a flicker rather than as a message opening (docs/sync-progress.md).
    ///
    /// The light appearance goes with the page rather than decorating it. The canvas is white in
    /// both themes because `base_css` pins the document to `color-scheme: light` (mail is authored
    /// for a white page) so the dark appearance's own label colours over it would be white on
    /// white. The chrome around it (header, toolbar, the remote-images banner) stays themed.
    ///
    /// Not `private`: ReadingView.swift's `body` is what draws it (Swift's `private` in an
    /// extension is file-scoped).
    /// The reading pane below the action row: the body on its page, with the header above it
    /// wherever that header is not travelling inside the body's own scroll view.
    ///
    /// Not `private`: ReadingView.swift's `body` is what draws it (Swift's `private` in an
    /// extension is file-scoped).
    @ViewBuilder
    var content: some View {
        #if os(iOS)
        // An HTML body carries the header inside its scroll view, so it must not also be drawn
        // above it; every other state is SwiftUI's own and scrolls with the header here.
        if carriesItsOwnHeader {
            page
        } else {
            ScrollView {
                VStack(spacing: 0) {
                    scrollingHeader
                    page
                }
            }
        }
        #else
        if remoteImagesOffered {
            RemoteImagesBanner { loadRemoteImages = true }
        }
        page
        #endif
    }

    #if os(iOS)
    /// Whether the body on screen is the hardened web view, which is the one body that brings a
    /// scroll view of its own for the header to ride (ReadingView.Scroll.swift).
    private var carriesItsOwnHeader: Bool {
        guard let body = bodySnapshot, !body.pending, !body.loadError else { return false }
        return !(body.html ?? "").isEmpty
    }
    #endif

    /// The body on the page the core says a message is drawn on.
    private var page: some View {
        bodyArea
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(parseHexColor(messageCanvas().background))
            .environment(\.colorScheme, .light)
    }

    @ViewBuilder
    private var bodyArea: some View {
        if let body = bodySnapshot {
            if body.pending {
                // The core publishes this only once an open has run long enough to be worth
                // announcing, so the indicator appears for a wait and never for a fast open.
                // It carries no body, so this has to come before the branches that read one:
                // an empty `pending` snapshot is not a message without content.
                VStack(spacing: 10) {
                    ProgressView()
                    Text(L10n.reading_loading()).font(.caption).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if body.loadError {
                VStack(spacing: 10) {
                    Image(systemName: "exclamationmark.triangle").foregroundStyle(.secondary)
                    Text(L10n.reading_load_error()).foregroundStyle(.secondary)
                    Button(L10n.action_retry()) { model.openMessage(message.account, message.key) }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let html = body.html, !html.isEmpty {
                htmlBody(html)
            } else if let plain = body.plain, !plain.isEmpty {
                plainBody(plain)
            } else {
                placeholder(L10n.reading_no_content())
            }
        } else {
            // Opened, and nothing to say yet. Not a spinner: the body usually arrives within a
            // few milliseconds, and one drawn on every open flickers rather than reassures. The
            // header above is already filled from the row that was tapped, and the page is
            // already drawn, so the pane reads as the message opening rather than as empty.
            Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    /// The sanitised HTML body in its hardened web view (ReadingWebView.swift), carrying the
    /// header inside its own scroll view on the touch clients.
    @ViewBuilder
    private func htmlBody(_ html: String) -> some View {
        #if os(iOS)
        SanitizedHTMLView(
            fragment: html,
            loadRemoteImages: loadRemoteImages,
            header: AnyView(scrollingHeader)
        )
        #else
        SanitizedHTMLView(fragment: html, loadRemoteImages: loadRemoteImages)
        #endif
    }

    /// A plain-text body: text on the touch clients, where the reading view's own `ScrollView`
    /// already holds it and the header above it, and a scroll of its own on macOS.
    @ViewBuilder
    private func plainBody(_ plain: String) -> some View {
        let text = Text(plain)
            .font(.body)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(readingInset)
        #if os(iOS)
        text
        #else
        ScrollView { text }
        #endif
    }

    private func placeholder(_ text: String) -> some View {
        Text(text).foregroundStyle(.secondary)
    }
}

/// The bar shown above a message that has remote images, which are blocked by default to
/// avoid tracking. Tapping "Load images" opts in for this message.
///
/// Not `private`: ReadingView.Scroll.swift puts it in the scrolling header.
struct RemoteImagesBanner: View {
    let onLoad: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "photo").foregroundStyle(.secondary)
            Text(L10n.reading_remote_blocked())
                .font(.caption)
            Spacer()
            Button(L10n.action_load_images(), action: onLoad)
        }
        .padding(.horizontal, readingInset)
        .padding(.vertical, 8)
        .background(.quaternary)
    }
}
