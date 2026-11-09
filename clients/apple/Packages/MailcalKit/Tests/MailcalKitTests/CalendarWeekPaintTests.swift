// The drawn grid's one copy of where a block is: drawn there, tapped there, announced there.
//
// A canvas has no view per block to place, so the rectangle is arithmetic over the core's unit-free
// geometry, and a tap and a screen reader both read it back. If they disagreed, a tap would open the
// wrong event, or VoiceOver would announce one somewhere it is not drawn (docs/calendar.md §7).

import CoreGraphics
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

private let hourHeight: CGFloat = 60
private let dayWidth: CGFloat = 100

private func segment(
    event: String = "evt",
    day: Int = 1,
    start: Int = 9 * 60,
    end: Int = 10 * 60,
    column: Int = 0,
    columns: Int = 1,
    participation: ResponseStatus = .accepted
) -> TimedSegment {
    TimedSegment(
        account: "acct",
        event: event,
        calendar: "work",
        title: event,
        day: UInt32(day),
        startMinutes: UInt32(start),
        endMinutes: UInt32(end),
        column: UInt32(column),
        columns: UInt32(columns),
        continuesBefore: false,
        continuesAfter: false,
        canWrite: true,
        canMove: true,
        occurrenceStart: "",
        participation: participation
    )
}

private func paint(_ segment: TimedSegment) -> CalendarBlockPaint {
    CalendarBlockPaint(segment: segment, calendars: [], dark: false, use24Hour: true)
}

private func week(_ segments: [TimedSegment]) -> CalendarWeekPaint {
    CalendarWeekPaint(
        page: CalendarPage(
            days: [], timed: segments, allDay: [], allDayLanes: 0,
            timezone: "Europe/Amsterdam", calendars: [], isMaterialized: true
        ),
        dark: false,
        use24Hour: true
    )
}

@Suite struct CalendarWeekPaintTests {

    @Test func aBlockSitsInItsDayAndLaneAtItsTimeLessTheGap() {
        // Tuesday, the second of two lanes, 09:00 to 10:30.
        let rect = paint(segment(day: 1, start: 540, end: 630, column: 1, columns: 2))
            .rect(dayWidth: dayWidth, hourHeight: hourHeight)
        #expect(rect == CGRect(
            x: 100 + 50 + blockGap, y: 540 + blockGap,
            width: 50 - blockGap * 2, height: 90 - blockGap * 2
        ))
    }

    @Test func aZoomMovesTheBlockAndNothingElse() {
        // The paint is built once per page; only the multiplication changes with the zoom.
        let block = paint(segment(start: 600, end: 660))
        let wide = block.rect(dayWidth: 200, hourHeight: 120)
        #expect(wide.minY == 1200 + blockGap)
        #expect(wide.height == 120 - blockGap * 2)
        #expect(block.time == "10:00")
    }

    @Test func aTapOpensTheBlockDrawnOnTop() {
        // The core orders blocks the way they are drawn, so the later one covers the earlier.
        let under = segment(event: "under", start: 540, end: 660)
        let over = segment(event: "over", start: 600, end: 630)
        let grid = week([under, over])
        let point = CGPoint(x: 150, y: 610)
        #expect(grid.block(at: point, dayWidth: dayWidth, hourHeight: hourHeight)?.event == "over")
        #expect(
            grid.block(at: CGPoint(x: 150, y: 560), dayWidth: dayWidth, hourHeight: hourHeight)?
                .event == "under"
        )
    }

    @Test func aTapOnEmptyGridOpensNothing() {
        let grid = week([segment(start: 540, end: 600)])
        #expect(grid.block(at: CGPoint(x: 150, y: 700), dayWidth: dayWidth, hourHeight: hourHeight) == nil)
        #expect(grid.block(at: CGPoint(x: 50, y: 560), dayWidth: dayWidth, hourHeight: hourHeight) == nil)
    }

    @Test func theBlockADragHoldsIsTheOneItLifted() {
        let block = paint(segment(event: "evt", day: 2))
        let lifted = CalendarDragSubject(
            account: "acct", event: "evt", occurrenceStart: "", day: 2, startMinutes: 540,
            endMinutes: 600
        )
        let another = CalendarDragSubject(
            account: "acct", event: "evt", occurrenceStart: "", day: 3, startMinutes: 540,
            endMinutes: 600
        )
        #expect(block.isHeld(by: lifted))
        #expect(!block.isHeld(by: another))
        #expect(!block.isHeld(by: nil))
    }

    @Test func anUnansweredInvitationIsDrawnAndSpokenAsAHold() {
        let hold = paint(segment(participation: .needsAction))
        let accepted = paint(segment(participation: .accepted))
        #expect(hold.hold && !accepted.hold)
        #expect(hold.spoken.hasSuffix(L10n.a11y_invitation_awaiting_response()))
    }

    @Test func anOccurrenceOfASeriesWearsTheRepeatGlyphAndSaysSo() {
        // The core names an occurrence of a series by its original start and leaves a one-off
        // event's empty. The glyph is hidden from VoiceOver, so the label carries it.
        var series = segment()
        series.occurrenceStart = "20261103T090000"
        let repeating = paint(series)
        let once = paint(segment())
        #expect(repeating.repeats && !once.repeats)
        #expect(repeating.spoken.hasSuffix(L10n.a11y_event_repeats()))
        #expect(!once.spoken.contains(L10n.a11y_event_repeats()))
    }

    @Test func aTitleIsMeasuredByItsWidestWord() {
        // The grid wraps a title only where every word fits its line: a word broken across two
        // ("hackatho / n") reads as a broken grid. So the decision rests on the widest word.
        let widest = widestWordWidth("hackathon soverin", size: 11)
        #expect(widest == widestWordWidth("hackathon", size: 11))
        #expect(widest > widestWordWidth("soverin", size: 11))
        #expect(widestWordWidth("", size: 11) == 0)
    }

    @Test func twoOccurrencesOfASeriesOnOneDayAreTwoBlocks() {
        // A series can occur twice in a day (every four hours), and both occurrences share an
        // account, an event and a day. Sharing an identity would draw the second with the first's
        // time, from the label cache, and give VoiceOver two elements with one id.
        let morning = paint(segment(event: "pills", start: 480, end: 495))
        let noon = paint(segment(event: "pills", start: 720, end: 735))
        #expect(morning.id != noon.id)
    }

    @Test func aBlockShowsItsTimeAsARange() {
        #expect(paint(segment(start: 540, end: 720)).range == "09:00–12:00")
    }
}
