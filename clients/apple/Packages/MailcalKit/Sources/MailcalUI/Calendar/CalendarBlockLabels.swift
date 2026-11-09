// A week's block labels, resolved and measured once rather than on every frame.
//
// Text is what a frame of the drawn grid costs: resolving a title, measuring it and laying it out,
// for every block, sixty times a second, measured at two thirds of the main thread during a zoom.
// None of that changes while the grid moves, and only the wrapping changes with a zoom. So each
// label is resolved once per paint, its fixed widths are kept with it, and a title's wrapping is
// laid out against its width rounded down to a step (`labelStep`), so a zoom finds the layout it
// made a frame ago instead of shaping the text again (docs/calendar.md §7: "stop it changing").

import SwiftUI

/// The step a title's wrapping width is rounded down to.
let labelStep: CGFloat = 4

/// One block's label at one type size.
final class CalendarBlockLabel {
    let title: GraphicsContext.ResolvedText
    let time: GraphicsContext.ResolvedText
    let range: GraphicsContext.ResolvedText
    let glyphs: GraphicsContext.ResolvedText?
    let glyphWidth: CGFloat
    let rangeWidth: CGFloat
    /// The height of one line of the title, which is where a whole line ends.
    let line: CGFloat
    private var wrapped: [Int: CGSize] = [:]

    init(_ block: CalendarBlockPaint, size: CGFloat, in context: GraphicsContext) {
        let unbounded = CGSize(width: CGFloat.infinity, height: .infinity)
        title = context.resolve(
            Text(block.title).font(.system(size: size, weight: .semibold)).foregroundColor(block.text)
        )
        time = context.resolve(Text(block.time).font(.system(size: size)).foregroundColor(block.text))
        range = context.resolve(Text(block.range).font(.system(size: size)).foregroundColor(block.text))
        glyphs = cornerGlyphs(block, size: size).map { context.resolve($0) }
        glyphWidth = glyphs?.measure(in: unbounded).width ?? 0
        rangeWidth = range.measure(in: unbounded).width
        line = title.measure(in: unbounded).height
    }

    /// The title's size wrapped to `width`, a multiple of `labelStep`.
    func wrapped(to width: CGFloat) -> CGSize {
        let key = Int(width / labelStep)
        if let held = wrapped[key] { return held }
        let size = title.measure(in: CGSize(width: width, height: .infinity))
        wrapped[key] = size
        return size
    }
}

/// The labels of one week's blocks, built as the canvas first draws each one.
///
/// Held by the week's paint, so a new calendar version, appearance or clock starts afresh.
/// `@unchecked` because it is used only from the canvas's drawing, which runs on the main thread:
/// the week canvas does not render asynchronously, and must not start to while this is unlocked.
final class CalendarBlockLabels: @unchecked Sendable {
    private var labels: [String: CalendarBlockLabel] = [:]

    func label(
        for block: CalendarBlockPaint, size: CGFloat, in context: GraphicsContext
    ) -> CalendarBlockLabel {
        let key = "\(block.id)|\(size)"
        if let held = labels[key] { return held }
        let label = CalendarBlockLabel(block, size: size, in: context)
        labels[key] = label
        return label
    }
}

/// The meeting and repeat glyphs, side by side, or nil when the event has neither.
func cornerGlyphs(_ block: CalendarBlockPaint, size: CGFloat) -> Text? {
    let names = (block.hasConference ? ["video"] : []) + (block.repeats ? ["repeat"] : [])
    guard !names.isEmpty else { return nil }
    var glyphs = Text(Image(systemName: names[0]))
    for name in names.dropFirst() { glyphs = glyphs + Text(" ") + Text(Image(systemName: name)) }
    return glyphs.font(.system(size: size * 0.85)).foregroundColor(block.text)
}
