// What a pane of the Mac window may ask of the window: its height is the window's, and the panes'
// minimum widths fit inside the window's minimum width.

#if os(macOS)
import SwiftUI

/// The widths the Mac window's panes are held to, in points. The window's minimum is derived from
/// them, so a pane cannot be given a minimum the window has no room for.
enum MacShellWidths {
    static let sidebar: ClosedRange<CGFloat> = 220...320
    static let messageList: CGFloat = 420
    static let reading: CGFloat = 420
    static let calendar: CGFloat = 620
    static let contactsList: CGFloat = 300
    static let contactsDetail: CGFloat = 420

    /// What each destination puts beside the sidebar.
    enum Destination: CaseIterable {
        case mail, calendar, contacts

        var paneMinimums: [CGFloat] {
            switch self {
            case .mail: [MacShellWidths.messageList, MacShellWidths.reading]
            case .calendar: [MacShellWidths.calendar]
            case .contacts: [MacShellWidths.contactsList, MacShellWidths.contactsDetail]
            }
        }
    }

    /// The sidebar at its widest beside the widest set of pane minimums. AppKit holds a split's
    /// panes to their minimums only while the window has room for all of them; past that it
    /// squeezes them below, and their content runs off the window's edge.
    static let windowMinimum: CGFloat = sidebar.upperBound
        + Destination.allCases.map { $0.paneMinimums.reduce(0, +) }.max()!
}

extension View {
    /// Takes the height the pane is offered, no more and no less, with its content at the top.
    ///
    /// `.frame(maxHeight: .infinity)` alone is not this: with no minimum, a frame answers with its
    /// content's height whenever that is taller than the offer, and the answer reaches the
    /// window's root through the split views, so one tall pane moves the whole window's content
    /// (docs/client-traps.md). Content taller than the pane is cut at its bottom edge, so a pane
    /// that can hold more than a window's height scrolls it itself.
    func macPaneHeight() -> some View {
        frame(minHeight: 0, maxHeight: .infinity, alignment: .top)
    }

    /// A window's root: at least `minWidth` by `minHeight`, which AppKit reads as the window's
    /// minimum size, and otherwise the window's own height, never its content's, for the reason
    /// `macPaneHeight` gives. Both height bounds sit in one frame: a minimum-only frame grows with
    /// its content, and a fill-only frame around it passes that growth on.
    func macWindowFrame(minWidth: CGFloat, minHeight: CGFloat) -> some View {
        frame(minWidth: minWidth, minHeight: minHeight, maxHeight: .infinity, alignment: .top)
    }
}
#endif
