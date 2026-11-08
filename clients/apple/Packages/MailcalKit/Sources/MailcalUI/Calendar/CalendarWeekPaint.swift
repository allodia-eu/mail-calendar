// What a week of the time grid wears, decided once per page rather than once per frame.
//
// A pinch changes the hour height and the column width on every frame, and nothing else: a block's
// colours, its title, its start time and what it says to a screen reader stay what they were. So they
// are worked out here, once per calendar version, and a frame only multiplies the core's unit-free
// geometry by this frame's hour and column (docs/calendar.md §7: "Nothing that a zoom cannot change
// may be re-derived on a zoom").

import MailcalBindings
import SwiftUI
#if canImport(UIKit)
    import UIKit
#else
    import AppKit
#endif

/// One block, ready to draw.
struct CalendarBlockPaint: Identifiable, Sendable {
    let id: String
    let account: String
    let event: String
    let occurrence: String
    let day: Int
    let column: Int
    let columns: Int
    let start: Int
    let end: Int
    /// The tint the block is filled with, fainter for a hold.
    let fill: Color
    /// The calendar's colour: the bar down the leading edge, and a hold's dashed edge and hatching.
    let accent: Color
    let text: Color
    /// An unanswered invitation: a fainter fill, a hatched leading edge and a dashed border.
    let hold: Bool
    /// One occurrence of a series, which earns the repeat glyph.
    let repeats: Bool
    let title: String
    let hasConference: Bool
    /// The start time, for a block too narrow for the range.
    let time: String
    /// "09:00–12:00", the second line a tall enough block earns.
    let range: String
    /// What a screen reader says for it, the hold, the meeting and the repeat included.
    let spoken: String
    /// The widest word of the title at each label size (`blockLabelFontSize`): a title wraps only
    /// where every word fits its line, because a word broken across two reads as a broken grid.
    let widestWord: (small: CGFloat, regular: CGFloat)

    var minutes: Int { end - start }

    var ref: EventRefID { EventRefID(account: account, key: event, occurrence: occurrence) }

    init(segment: TimedSegment, calendars: [CalendarRow], dark: Bool, use24Hour: Bool) {
        let calendar = calendars.row(account: segment.account, calendar: segment.calendar)
        let colors = calendar.eventColorsOrFallback(dark: dark)
        let title = segment.title.isEmpty ? L10n.event_no_title() : segment.title
        // The segment's own key, plus its start: one series can occur twice on a day, and both
        // occurrences share an account, an event and a day.
        id = "\(segment.rowID):\(segment.startMinutes)"
        account = segment.account
        event = segment.event
        occurrence = segment.occurrenceStart
        day = Int(segment.day)
        column = Int(segment.column)
        columns = max(Int(segment.columns), 1)
        start = Int(segment.startMinutes)
        end = Int(segment.endMinutes)
        hold = isAwaitingResponse(segment.participation)
        fill = colors.fill.opacity(hold ? holdFillOpacity : 1)
        accent = colors.accent
        text = colors.text
        // The core names an occurrence of a series by its original start, and leaves a one-off
        // event's empty: the same signal a drag asks "this event, or all of them?" on.
        repeats = !segment.occurrenceStart.isEmpty
        self.title = title
        hasConference = segment.hasConference
        time = clockTime(start, use24Hour: use24Hour)
        range = "\(time)–\(clockTime(end, use24Hour: use24Hour))"
        widestWord = (
            small: widestWordWidth(title, size: blockLabelFontSize(minutes: 0)),
            regular: widestWordWidth(title, size: blockLabelFontSize(minutes: 60))
        )
        spoken = calendarEventLabel(
            title: title,
            time: timeRange(start, end, use24Hour: use24Hour),
            calendar: calendar?.name ?? "",
            participation: segment.participation,
            hasConference: segment.hasConference,
            repeats: repeats
        )
    }

    /// Where the block is drawn, hit and announced: the one copy of that arithmetic.
    func rect(dayWidth: CGFloat, hourHeight: CGFloat) -> CGRect {
        let columnWidth = dayWidth / CGFloat(columns)
        return CGRect(
            x: dayWidth * CGFloat(day) + columnWidth * CGFloat(column) + blockGap,
            y: hourHeight * (CGFloat(start) / 60) + blockGap,
            width: max(columnWidth - blockGap * 2, 0),
            height: max(hourHeight * (CGFloat(minutes) / 60) - blockGap * 2, 0)
        )
    }

    /// Whether a drag has this block in its hand.
    func isHeld(by subject: CalendarDragSubject?) -> Bool {
        guard let subject else { return false }
        return subject.account == account && subject.event == event && subject.day == day
    }
}

/// The width of the widest word in `title`, set semibold at `size`, as the grid draws a title.
func widestWordWidth(_ title: String, size: CGFloat) -> CGFloat {
    #if canImport(UIKit)
        let font = UIFont.systemFont(ofSize: size, weight: .semibold)
    #else
        let font = NSFont.systemFont(ofSize: size, weight: .semibold)
    #endif
    return title.split(whereSeparator: \.isWhitespace)
        .map { String($0).size(withAttributes: [.font: font]).width }
        .max() ?? 0
}

/// A week's blocks, in the core's order, which is the order they are drawn in. Immutable, so the
/// equality check that compares weeks by paint can read it from any actor.
final class CalendarWeekPaint: Sendable {
    let blocks: [CalendarBlockPaint]
    /// The blocks' labels, resolved as the canvas first draws them (`CalendarBlockLabels`).
    let labels = CalendarBlockLabels()

    init(page: CalendarPage, dark: Bool, use24Hour: Bool) {
        blocks = page.timed.map {
            CalendarBlockPaint(segment: $0, calendars: page.calendars, dark: dark, use24Hour: use24Hour)
        }
    }

    /// The block under `point`, the topmost where two are drawn over each other.
    func block(at point: CGPoint, dayWidth: CGFloat, hourHeight: CGFloat) -> CalendarBlockPaint? {
        blocks.last { $0.rect(dayWidth: dayWidth, hourHeight: hourHeight).contains(point) }
    }
}

/// The weeks the grid has painted, held until the calendar, the appearance or the clock changes.
final class CalendarWeekPaints {
    private struct Stamp: Equatable {
        let version: Int
        let dark: Bool
        let use24Hour: Bool
    }

    /// The same reach as `CalendarPageCache`, whose pages these are built from.
    private static let capacity = 32

    private var stamp: Stamp?
    private var paints: [String: CalendarWeekPaint] = [:]

    func paint(
        for week: CalendarStripWeek, version: Int, dark: Bool, use24Hour: Bool
    ) -> CalendarWeekPaint {
        let current = Stamp(version: version, dark: dark, use24Hour: use24Hour)
        if stamp != current {
            stamp = current
            paints.removeAll(keepingCapacity: true)
        }
        let key = week.page.days.first?.date ?? ""
        if let held = paints[key] { return held }
        let paint = CalendarWeekPaint(page: week.page, dark: dark, use24Hour: use24Hour)
        if paints.count >= Self.capacity { paints.removeAll(keepingCapacity: true) }
        paints[key] = paint
        return paint
    }
}
