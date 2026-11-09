// The grid's fixed furniture: the pinned hour ruler, the hour and day lines, the now line, and the
// block a drag has in its hand. Split out of CalendarGridView.swift to keep it under 500 lines.

import MailcalBindings
import SwiftUI

/// The hour ruler. Each label straddles its gridline so "09" reads as the 09:00 boundary rather than
/// as a name for the band beneath it.
struct CalendarHourRuler: View {
    let use24Hour: Bool
    let hourHeight: CGFloat

    var body: some View {
        VStack(spacing: 0) {
            ForEach(0..<calendarHours, id: \.self) { hour in
                ZStack(alignment: .topTrailing) {
                    Color.clear
                    Text(hourLabel(hour, use24Hour: use24Hour))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .offset(y: -6)
                        .padding(.trailing, 6)
                }
                .frame(width: calendarGutter, height: hourHeight)
            }
        }
    }
}

/// The hour lines and the day dividers, drawn once rather than as ~30 composed views.
struct CalendarGridLines: View {
    let dayCount: Int
    let dayWidth: CGFloat
    let hourHeight: CGFloat

    var body: some View {
        Canvas { context, size in
            drawCalendarGridLines(
                in: context, size: size, dayCount: dayCount, dayWidth: dayWidth,
                hourHeight: hourHeight
            )
        }
    }
}

/// The hour lines and the day dividers, into a canvas the grid is already drawing.
func drawCalendarGridLines(
    in context: GraphicsContext, size: CGSize, dayCount: Int, dayWidth: CGFloat, hourHeight: CGFloat
) {
    let line = Color.secondary.opacity(0.25)
    var path = Path()
    for hour in 1..<calendarHours {
        let y = hourHeight * CGFloat(hour)
        path.move(to: CGPoint(x: 0, y: y))
        path.addLine(to: CGPoint(x: size.width, y: y))
    }
    // From zero, not from one: with the weeks laid end to end there is no gutter between them, so a
    // week's first line is the seam it shares with the week before it.
    for day in 0..<dayCount {
        let x = dayWidth * CGFloat(day)
        path.move(to: CGPoint(x: x, y: 0))
        path.addLine(to: CGPoint(x: x, y: size.height))
    }
    context.stroke(path, with: .color(line), lineWidth: 0.5)
}

/// The block a drag has in its hand, drawn where the pointer has it, with the time it would land on
/// written on it. Positioned in its own week's coordinates, like every other block.
///
/// The label is the point: a quarter-hour snap is invisible on a zoomed-out grid (the block moves
/// three points and the user has no way to know whether that was 15 minutes or 30).
struct CalendarDragBlock: View {
    let drag: CalendarDragState
    /// The page of the week the drag began in, or nil if the core has none for it.
    let page: CalendarPage?
    let dayWidth: CGFloat
    let hourHeight: CGFloat
    let use24Hour: Bool
    let dark: Bool

    var body: some View {
        // Two currencies, on purpose. The block is drawn from `livePreview()`, which follows the
        // pointer to the minute, so the motion is smooth instead of stepping a quarter-hour at a
        // time. The readout is drawn from `preview()`, which is snapped, it is the number that will
        // be written, and a readout agreeing with the pixels instead would quote a minute the drop
        // cannot honour.
        let live = drag.livePreview()
        let settled = drag.preview()
        // A held block takes its whole column: the core's lane packing describes where it *was*, and
        // re-solving overlaps per frame is work a gesture must not do.
        let held = page?.timed.first { segment in
            guard let subject = drag.subject else { return false }
            return segment.account == subject.account && segment.event == subject.event
                && Int(segment.day) == subject.day
        }
        // A new slot wears the calendar it would be filed on; a held one keeps its own colours. The
        // accent it used to fall back to is the one colour on the grid that means nothing: every
        // other block says which calendar it belongs to, and the one being created, the only one
        // whose calendar is still a choice, said "accent" on its way to a red calendar.
        let colors = (held.flatMap { segment in
            page?.calendars.row(account: segment.account, calendar: segment.calendar)
        } ?? page?.calendars.first { $0.isDefault }).map { eventColors(hex: $0.color.hex, dark: dark) }

        let blockWidth = max(dayWidth - 2, 0)
        let blockHeight = max(hourHeight * CGFloat(live.minutes) / 60 - 2, 0)
        let blockX = dayWidth * CGFloat(live.day) + 1
        let blockY = hourHeight * CGFloat(live.startMinutes) / 60 + 1

        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: blockCorner)
                .fill(colors?.fill ?? Color.accentColor.opacity(0.2))
                .overlay(
                    RoundedRectangle(cornerRadius: blockCorner)
                        .strokeBorder(colors?.accent ?? Color.accentColor, lineWidth: 2)
                )
                .frame(width: blockWidth, height: blockHeight)
                .offset(x: blockX, y: blockY)

            // The time floats beside the block rather than being written inside it. Inside, it was
            // dropped exactly when it was needed most: a fifteen-minute slot at a zoomed-out horizon
            // is a few points tall, and the one label that tells a 15-minute snap from a 30-minute
            // one did not fit in it.
            CalendarDragReadout(
                text: timeRange(settled.startMinutes, settled.endMinutes, use24Hour: use24Hour),
                dark: dark
            )
            .offset(x: blockX, y: max(blockY - dragReadoutGap, 0))
        }
        .allowsHitTesting(false)
    }
}

/// The gap between the block and the readout floating above it.
private let dragReadoutGap: CGFloat = 22

/// The time a drag would land on, in a floating pill.
///
/// The inverse of the surface it sits on, the way a tooltip is, stated as two hexes rather than
/// taken from a system colour, because the pair has to exist on macOS and iOS alike and the
/// platform-specific ones do not.
struct CalendarDragReadout: View {
    let text: String
    let dark: Bool

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .foregroundStyle(parseHexColor(dark ? "#1b1b1b" : "#f2f2f2"))
            .background(parseHexColor(dark ? "#e3e3e3" : "#303030"), in: Capsule())
    }
}
