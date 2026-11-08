// One week of the time grid, drawn rather than composed (docs/calendar.md §7).
//
// The grid lines, every block and the now line are one `Canvas`, so a pinch costs one draw per week
// rather than a layout pass per block, and a week sliding into view is one view rather than sixty.
// A frame multiplies the core's geometry by this frame's hour and column and strokes rectangles and
// short strings.
//
// A canvas has no accessibility tree, so the week carries one invisible element per block while an
// assistive technology is running, placed by the same `CalendarBlockPaint.rect` the drawing and the
// tap use, so what is announced is where it is drawn.

import MailcalBindings
import SwiftUI

/// One week's grid lines, blocks and now line, in its own coordinates.
///
/// Equatable on what it draws, because most frames of a scroll or a landing move only where the week
/// sits, and that offset is applied outside this view. The paint is compared by identity: a new one
/// is made only when the page, the appearance or the clock changes. `onOpen` is not compared; it
/// opens whichever event it is handed.
struct CalendarWeekContent: View, Equatable {
    let index: Int
    let paint: CalendarWeekPaint
    let dayCount: Int
    let todayColumn: Int?
    let nowMinutes: Int
    /// The block a drag has lifted out of this week, drawn by `CalendarDragBlock` instead.
    let held: CalendarDragSubject?
    let dayWidth: CGFloat
    let hourHeight: CGFloat
    let onOpen: (EventRefID) -> Void

    @Environment(\.accessibilityEnabled) private var accessibilityEnabled

    nonisolated static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.index == rhs.index && lhs.paint === rhs.paint && lhs.dayCount == rhs.dayCount
            && lhs.todayColumn == rhs.todayColumn && lhs.nowMinutes == rhs.nowMinutes
            && lhs.held == rhs.held && lhs.dayWidth == rhs.dayWidth
            && lhs.hourHeight == rhs.hourHeight
    }

    private var size: CGSize {
        CGSize(width: dayWidth * CGFloat(dayCount), height: hourHeight * CGFloat(calendarHours))
    }

    var body: some View {
        CalendarFrameTrace.shared.weekBuilt()
        return Canvas { context, size in
            drawCalendarGridLines(
                in: context, size: size, dayCount: dayCount, dayWidth: dayWidth,
                hourHeight: hourHeight
            )
            // The block being dragged is drawn once, by `CalendarDragBlock`, where the pointer has
            // it, never twice. Leaving the original in place reads as a duplicate event.
            for block in paint.blocks where !block.isHeld(by: held) {
                draw(block, in: context)
            }
            if let todayColumn { drawNowLine(in: context, today: todayColumn, width: size.width) }
        }
        .frame(width: size.width, height: size.height)
        // Hit-testable only where a block is drawn, so empty grid stays with the grid's own gesture:
        // a press there draws out a new event, and a finger there pans.
        .contentShape(
            CalendarBlockHitShape(paint: paint, held: held, dayWidth: dayWidth, hourHeight: hourHeight)
        )
        .onTapGesture { point in
            if let block = paint.block(at: point, dayWidth: dayWidth, hourHeight: hourHeight),
               !block.isHeld(by: held) {
                onOpen(block.ref)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityChildren {
            if accessibilityEnabled { spokenWeek }
        }
    }

    // MARK: - Drawing

    private func draw(_ block: CalendarBlockPaint, in context: GraphicsContext) {
        let rect = block.rect(dayWidth: dayWidth, hourHeight: hourHeight)
        guard rect.width > 0, rect.height > 0 else { return }
        let shape = Path(roundedRect: rect, cornerRadius: blockCorner)
        context.fill(shape, with: .color(block.fill))

        var inside = context
        inside.clip(to: shape)
        // The calendar's colour down the leading edge. A hold hatches it instead, and dashes its
        // edge: provisional, where a commitment is solid.
        let edge = CGRect(x: rect.minX, y: rect.minY, width: eventBarWidth, height: rect.height)
        if block.hold {
            var hatch = inside
            hatch.clip(to: Path(edge.insetBy(dx: -0.5, dy: 0)))
            hatch.translateBy(x: edge.minX, y: edge.minY)
            hatch.stroke(holdHatchPath(size: edge.size), with: .color(block.accent), lineWidth: 1)
            context.stroke(
                Path(roundedRect: rect.insetBy(dx: 0.5, dy: 0.5), cornerRadius: blockCorner - 0.5),
                with: .color(block.accent),
                style: StrokeStyle(lineWidth: 1, dash: [3, 2])
            )
        } else {
            inside.fill(Path(edge), with: .color(block.accent))
        }
        drawLabel(block, in: inside, rect: rect)
    }

    /// The title, wrapped over every whole line the block has room for, then the time beneath it,
    /// with the meeting and repeat glyphs in the top-right corner. A block too short for one line of
    /// text draws none: a title cut through the middle reads as a broken grid (§4).
    private func drawLabel(_ block: CalendarBlockPaint, in context: GraphicsContext, rect: CGRect) {
        let minutes = block.minutes
        let lines = blockTitleLines(minutes: minutes, hourHeight: hourHeight)
        guard lines > 0 else { return }
        let size = blockLabelFontSize(minutes: minutes)
        let pitch = blockLabelLineHeight(minutes: minutes)
        let top = rect.minY + blockInset(minutes: minutes) - blockGap
        let left = rect.minX + eventBarWidth + 4
        let width = max(rect.maxX - 4 - left, 0)

        let label = paint.labels.label(for: block, size: size, in: context)

        // In the top-right corner, where Apple's Calendar keeps its own, so the title wraps beside
        // them rather than chasing them; and only where the title keeps room of its own, since a
        // glyph is worth less than the name of the event.
        var available = width
        if let glyphs = label.glyphs, width - label.glyphWidth - 3 >= minimumTitleWidth {
            available = width - label.glyphWidth - 3
            context.draw(glyphs, at: CGPoint(x: left + width, y: top + 1), anchor: .topTrailing)
        }
        let widest = size < blockLabelFontSize(minutes: 60)
            ? block.widestWord.small : block.widestWord.regular
        // Laid out against a width on `labelStep`'s grid, never wider than the room there is.
        let wrapWidth = (available / labelStep).rounded(.down) * labelStep
        let titleHeight: CGFloat
        var clipped = context
        if widest <= wrapWidth {
            // Wrapped by word, and cut after the last whole line there is room for: clipped at the
            // font's own line height, not the pitch the room was counted in, so nothing of the line
            // after the last whole one shows beneath it.
            let wrapped = label.wrapped(to: wrapWidth)
            titleHeight = min(wrapped.height, CGFloat(lines) * label.line)
            clipped.clip(to: Path(CGRect(x: left, y: top, width: available, height: titleHeight)))
            clipped.draw(
                label.title,
                in: CGRect(x: left, y: top, width: wrapWidth, height: wrapped.height)
            )
        } else {
            // A word wider than the block would be broken across two lines, so the title runs on as
            // one line and is cut at the block's edge instead.
            titleHeight = label.line
            clipped.clip(to: Path(CGRect(x: left, y: top, width: available, height: label.line)))
            clipped.draw(label.title, at: CGPoint(x: left, y: top), anchor: .topLeading)
        }

        guard blockShowsTime(minutes: minutes, hourHeight: hourHeight) else { return }
        // The range where it fits, the start alone where the column is too narrow for it.
        var below = context
        let timeRect = CGRect(x: left, y: top + titleHeight, width: width, height: pitch)
        below.clip(to: Path(timeRect))
        below.draw(
            label.rangeWidth <= width ? label.range : label.time,
            at: timeRect.origin,
            anchor: .topLeading
        )
    }

    /// The narrowest a title may be squeezed to make room for the corner glyphs.
    private var minimumTitleWidth: CGFloat { 36 }

    /// The red now line, with a dot on today's column.
    private func drawNowLine(in context: GraphicsContext, today: Int, width: CGFloat) {
        let y = hourHeight * (CGFloat(nowMinutes) / 60)
        context.fill(Path(CGRect(x: 0, y: y, width: width, height: 1.5)), with: .color(.red))
        context.fill(
            Path(ellipseIn: CGRect(x: dayWidth * CGFloat(today) - 4, y: y - 3, width: 8, height: 8)),
            with: .color(.red)
        )
    }

    // MARK: - Speaking

    /// One element per block, and the now line, where the canvas draws them.
    private var spokenWeek: some View {
        ZStack(alignment: .topLeading) {
            Color.clear
            ForEach(paint.blocks.filter { !$0.isHeld(by: held) }) { block in
                let rect = block.rect(dayWidth: dayWidth, hourHeight: hourHeight)
                Color.clear
                    .frame(width: rect.width, height: rect.height)
                    .position(x: rect.midX, y: rect.midY)
                    .accessibilityElement()
                    .accessibilityLabel(block.spoken)
                    .accessibilityAddTraits(.isButton)
                    .accessibilityAction { onOpen(block.ref) }
            }
            if let todayColumn {
                Color.clear
                    .frame(width: dayWidth, height: 1.5)
                    .position(
                        x: dayWidth * (CGFloat(todayColumn) + 0.5),
                        y: hourHeight * (CGFloat(nowMinutes) / 60)
                    )
                    .accessibilityElement()
                    .accessibilityLabel(L10n.calendar_now())
            }
        }
        .frame(width: size.width, height: size.height)
    }
}

/// The blocks a week draws, as one shape: what a tap on the week may land on.
private struct CalendarBlockHitShape: Shape {
    let paint: CalendarWeekPaint
    let held: CalendarDragSubject?
    let dayWidth: CGFloat
    let hourHeight: CGFloat

    func path(in _: CGRect) -> Path {
        var path = Path()
        for block in paint.blocks where !block.isHeld(by: held) {
            path.addRect(block.rect(dayWidth: dayWidth, hourHeight: hourHeight))
        }
        return path
    }
}
