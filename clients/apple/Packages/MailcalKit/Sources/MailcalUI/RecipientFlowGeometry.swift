// Where the recipient pills sit: left to right, wrapping onto a new line when the next one does not
// fit.
//
// A plain function over sizes rather than a method on the `Layout`, so the test suite drives it
// directly, `Layout.Subviews` cannot be constructed outside SwiftUI, which is what left this
// geometry uncovered while it was shipping a composer that walked off the screen.

import CoreGraphics

/// Lays `sizes` out in a row that wraps at `limit`, returning the container size and each item's
/// origin relative to it.
///
/// **Nothing is ever wider than `limit`.** An address longer than the field is the case that
/// matters: measured unconstrained it reports its full ideal width, and a container that then
/// claims that width is wider than the screen it sits in, the parent centres the overflow, which
/// takes every row off both edges and puts the pill's remove button past the tappable area.
func recipientFlowGeometry(
    sizes: [CGSize],
    limit: CGFloat,
    spacing: CGFloat
) -> (size: CGSize, positions: [CGPoint]) {
    var x: CGFloat = 0
    var y: CGFloat = 0
    var lineHeight: CGFloat = 0
    var widest: CGFloat = 0
    var positions: [CGPoint] = []
    for size in sizes {
        let width = min(size.width, limit)
        // `x > 0` keeps the first item on the line it starts: something too wide to fit anywhere
        // wraps forever otherwise, and it is already clamped to the line's width.
        if x > 0, x + width > limit {
            x = 0
            y += lineHeight + spacing
            lineHeight = 0
        }
        positions.append(CGPoint(x: x, y: y))
        x += width + spacing
        widest = max(widest, x - spacing)
        lineHeight = max(lineHeight, size.height)
    }
    return (CGSize(width: min(widest, limit), height: y + lineHeight), positions)
}
