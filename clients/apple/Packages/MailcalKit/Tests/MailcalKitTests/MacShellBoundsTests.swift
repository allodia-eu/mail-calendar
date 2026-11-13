// What a pane of the Mac window may ask of the window.
//
// The panes sit side by side in one window, so a pane that reports more height than the window has
// makes the whole window's content overflow, and SwiftUI centres what overflows: the command bar
// slides under the title bar and the sidebar's pinned rows drop off the bottom, beside a pane that
// did nothing. Across the width, pane minimums that add up to more than the window's minimum get
// squeezed below them by AppKit, and the reading header runs off the window's right edge.

#if os(macOS)
import AppKit
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct MacShellBoundsTests {
    /// A column like the reading pane's: a rigid header far taller than the window, over a body that
    /// takes whatever is left.
    private var tallColumn: some View {
        VStack(spacing: 0) {
            Color.blue.frame(height: 2_000)
            Color.gray.frame(maxHeight: .infinity)
        }
    }

    private func height(of view: some View, offered: CGFloat) -> CGFloat {
        NSHostingController(rootView: view)
            .sizeThatFits(in: CGSize(width: 800, height: offered)).height
    }

    @Test func aFrameThatOnlyFillsAnswersWithItsContentsHeight() {
        // The trap, stated so this suite fails if SwiftUI ever stops behaving this way and the
        // bound below stops being the reason the window holds still.
        let unbounded = tallColumn.frame(maxWidth: .infinity, maxHeight: .infinity)
        #expect(height(of: unbounded, offered: 500) == 2_000)
    }

    @Test func aBoundedPaneAnswersWithTheHeightItIsOffered() {
        #expect(height(of: tallColumn.macPaneHeight(), offered: 500) == 500)
        #expect(height(of: tallColumn.macPaneHeight(), offered: 900) == 900)
    }

    @Test func aBoundedPaneStillFillsATallWindow() {
        #expect(height(of: Color.clear.frame(height: 40).macPaneHeight(), offered: 900) == 900)
    }

    @Test func aWindowRootAnswersWithTheWindowsHeightAndKeepsItsMinimum() {
        let root = tallColumn.macWindowFrame(minWidth: 520, minHeight: 400)
        #expect(height(of: root, offered: 500) == 500)
        // What AppKit asks for the window's minimum size.
        #expect(height(of: root, offered: 0) == 400)
    }

    @Test(arguments: MacShellWidths.Destination.allCases)
    func everyDestinationsPanesFitTheSmallestWindow(_ destination: MacShellWidths.Destination) {
        // The sidebar at its widest, every other pane at its narrowest: the one layout a person can
        // drag the window into that the window's minimum has to hold.
        let needed = MacShellWidths.sidebar.upperBound + destination.paneMinimums.reduce(0, +)
        #expect(needed <= MacShellWidths.windowMinimum)
    }
}
#endif
