// The touch clients' reading layout: the action row stays where the reader left it and everything
// else about the message, who sent it, who else got it, what it carries, scrolls away with the
// body it belongs to.
//
// iPhone and iPad only. macOS keeps a header that stands still above the body, because the scroll
// the two would have to share is the web view's own and `WKWebView` exposes no scroll view there
// (docs/reading-zoom.md records the same limit for fitting a message to the pane).

#if os(iOS)
import SwiftUI

extension ReadingView {
    /// Everything about the message that is not the message: it belongs to the mail rather than to
    /// the pane, so it goes up with it.
    ///
    /// Where it then *lives* depends on what the body is. An HTML body brings a scroll view of its
    /// own, and the header is mounted inside it (`SanitizedHTMLView`), because a scroll view's pan
    /// gesture reaches only what is inside it: drawn over the message instead, the header would
    /// take every drag that landed on it, and one that fills the screen, an invitation card or
    /// twenty attachments, could not be scrolled past at all. Everything else, a plain-text note
    /// and the states of an open that carry no body, is SwiftUI's own and scrolls with the header
    /// in an ordinary `ScrollView` (ReadingView.Body.swift).
    var scrollingHeader: some View {
        VStack(spacing: 0) {
            identityHeader
            recipientsHeader
            attachmentBar
            invitationCard
            if remoteImagesOffered {
                RemoteImagesBanner { loadRemoteImages = true }
            }
            Divider()
        }
    }
}
#endif
