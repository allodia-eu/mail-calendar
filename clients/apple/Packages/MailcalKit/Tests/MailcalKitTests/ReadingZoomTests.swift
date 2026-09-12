// The reader's zoom on a message body (docs/reading-zoom.md).
//
// Two rules, and the second is the one that fails quietly. macOS's `magnification` is the *view's*
// rather than the page's, so it survives `loadHTMLString`: drop the reset and every message after a
// zoomed one opens at a scale chosen for a different message, which looks like the app forgetting
// its own layout rather than like a missing line. Nothing about that is visible in a screenshot of
// the message that was zoomed on purpose.
//
// `swift test` runs on macOS, so an iOS suite here would compile to nothing and report a pass over
// exactly the code in question; the iPhone/iPad half of rule 5 is the page scale's own reset on
// load, which is WebKit's rather than ours, and is verified on the simulator.

import Testing
import WebKit

@testable import MailcalUI

#if os(macOS)
@MainActor
@Suite struct ReadingZoomTests {

    @Test func theReaderCanPinchAMessage() {
        #expect(makeReadingWebView().allowsMagnification)
    }

    @Test func scriptingStaysOffWhereverTheHostIsBuilt() {
        // The zoom seam is on the same path as the rendering gates, so it is the place a later
        // refactor could drop one; rendering-security.md's Layer 3 gate 1, restated where it lives.
        let webView = makeReadingWebView()
        #expect(webView.configuration.defaultWebpagePreferences.allowsContentJavaScript == false)
    }

    @Test func openingAnotherMessageStartsAgainAtItsOwnFit() {
        let webView = makeReadingWebView()
        webView.magnification = 2.5

        loadReadingDocument(webView, "<!doctype html><html><body>another message</body></html>")

        #expect(webView.magnification == 1)
    }
}
#endif
