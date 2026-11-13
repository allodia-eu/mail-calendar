// A block at the top of a scrolled page that changes height out of sight moves the scroll view
// with it, so what the person was looking at stays where it was. The block is an account page's
// notices, which the core raises and clears as a change on the page runs. AppKit's scroll view
// keeps its offset as content above it grows, so without this everything below such a block moves
// by its height, and a form just opened can end up below the visible edge.

import SwiftUI

enum ScrolledPast {
    /// How far to move the scroll view when a block's frame, in the scroll view's coordinates, went
    /// from `old` to `new`. `nil` when its height did not change, or when any of it was in sight:
    /// the person then sees it change, and what is below it moving is what they expect.
    static func shift(from old: CGRect, to new: CGRect) -> CGFloat? {
        guard old.maxY <= 0, new.height != old.height else { return nil }
        return new.height - old.height
    }
}

extension EnvironmentValues {
    /// Moves the enclosing scroll view by a number of points. `nil` where none answers it.
    @Entry var scrollBy: ScrollBy?
}

struct ScrollBy {
    let move: @MainActor (CGFloat) -> Void
}

extension View {
    /// Keeps what is on screen in place when this block changes height out of sight.
    func keepsPlaceAbove() -> some View {
        modifier(KeepsPlaceAbove())
    }

    /// Answers `scrollBy` for the content of this scroll view.
    func answersScrollBy() -> some View {
        modifier(AnswersScrollBy())
    }
}

private struct KeepsPlaceAbove: ViewModifier {
    @Environment(\.scrollBy) private var scrollBy

    func body(content: Content) -> some View {
        content.onGeometryChange(for: CGRect.self) { $0.frame(in: .scrollView) } action: { old, new in
            if let shift = ScrolledPast.shift(from: old, to: new) { scrollBy?.move(shift) }
        }
    }
}

private struct AnswersScrollBy: ViewModifier {
    @State private var position = ScrollPosition(edge: .top)
    @State private var offset: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .scrollPosition($position)
            .onScrollGeometryChange(for: CGFloat.self) { $0.contentOffset.y } action: { _, y in
                offset = y
            }
            .environment(\.scrollBy, ScrollBy { shift in position.scrollTo(y: offset + shift) })
    }
}
