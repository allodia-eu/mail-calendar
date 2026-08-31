// Dragging on the grid: the decisions, without a pointer.
//
// The gesture itself has to be tried by hand, that is what §10 of docs/calendar.md says and it is
// true here too. But everything that can actually be *wrong* about a drag is arithmetic: what a
// press meant, where a drop would leave the block, and what crosses the FFI. All of that is here.
//
// These are deliberately the same cases, with the same names, as Android's `CalendarDragTest`.
// Drag is a **cross-platform contract**, and two clients that agree in prose but not in arithmetic
// have not kept it.

import CoreGraphics
import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

/// A geometry with round numbers: an hour is 60pt tall and a day 100pt wide, so a point is a minute.
private let hourHeight: CGFloat = 60
private let dayWidth: CGFloat = 100

private func segment(
    day: Int = 1,
    startMinutes: Int = 9 * 60,
    endMinutes: Int = 10 * 60,
    canMove: Bool = true,
    occurrenceStart: String = "",
    continuesBefore: Bool = false,
    continuesAfter: Bool = false
) -> TimedSegment {
    TimedSegment(
        account: "acct",
        event: "evt",
        calendar: "work",
        title: "Standup",
        day: UInt32(day),
        startMinutes: UInt32(startMinutes),
        endMinutes: UInt32(endMinutes),
        column: 0,
        columns: 1,
        continuesBefore: continuesBefore,
        continuesAfter: continuesAfter,
        canWrite: true,
        canMove: canMove,
        occurrenceStart: occurrenceStart,
        participation: .accepted
    )
}

/// A content-space point in day column `day` at wall-clock `minute`.
private func point(day: Int, minute: Int, inset: CGFloat = 0) -> CGPoint {
    CGPoint(
        x: dayWidth * CGFloat(day) + dayWidth / 2,
        y: hourHeight * CGFloat(minute) / 60 + inset
    )
}

private func press(
    _ at: CGPoint, segments: [TimedSegment] = [segment()], canCreate: Bool = true
) -> CalendarDragState? {
    calendarDrag(
        at: at, segments: segments, dayCount: 7,
        dayWidth: dayWidth, hourHeight: hourHeight, canCreate: canCreate
    )
}

@Suite struct CalendarDragDecisionTests {

    @Test func aPressInTheMiddleOfOurOwnBlockMovesIt() {
        let drag = press(point(day: 1, minute: 9 * 60 + 30))
        #expect(drag?.kind == .move)
        #expect(drag?.subject?.event == "evt")
    }

    @Test func aPressNearAnEdgeResizesThatEdgeAndAnchorsOnIt() {
        // Anchoring on the *edge* rather than on the pointer is what stops the first frame jumping
        // the edge to wherever inside the grab zone the pointer happened to land.
        let top = press(point(day: 1, minute: 9 * 60, inset: 3))
        #expect(top?.kind == .resizeStart)
        #expect(top?.anchorMinute == 9 * 60)

        let bottom = press(point(day: 1, minute: 10 * 60, inset: -3))
        #expect(bottom?.kind == .resizeEnd)
        #expect(bottom?.anchorMinute == 10 * 60)
    }

    @Test func aBlockTooShortForTwoGrabZonesIsAlwaysAMove() {
        // A quarter-hour block here is 15pt tall and each zone is 8pt: applied literally, every
        // press on it would be a resize of something whose middle you cannot reach.
        let short = [segment(startMinutes: 9 * 60, endMinutes: 9 * 60 + 15)]
        #expect(press(point(day: 1, minute: 9 * 60, inset: 2), segments: short)?.kind == .move)
    }

    @Test func aMeetingWeDoNotOwnIsNotPickedUp() {
        // The core's answer, not ours: `canMove` is narrower than "the calendar is writable". A
        // press on somebody else's meeting falls through to a create, exactly as a press on bare
        // grid does, doing nothing at all reads as the app having missed the gesture.
        let drag = press(point(day: 1, minute: 9 * 60 + 30), segments: [segment(canMove: false)])
        #expect(drag?.kind == .create)
        #expect(drag?.subject == nil)
    }

    @Test func aSegmentClippedByMidnightIsNotPickedUp() {
        // Its visible rectangle is a clip of the event, not the event: every gesture on it would
        // mean something other than what it looks like.
        let overnight = [segment(startMinutes: 0, endMinutes: 8 * 60, continuesBefore: true)]
        #expect(press(point(day: 1, minute: 4 * 60), segments: overnight)?.kind == .create)
    }

    @Test func aPressCreatesNothingWhenNoCalendarCanBeWritten() {
        // The same gate the "New event" button is disabled by: drawing out a slot that can never be
        // filed anywhere is an affordance that cannot fire.
        #expect(press(point(day: 3, minute: 14 * 60), segments: [], canCreate: false) == nil)
    }

    @Test func aPointPastTheLastColumnIsNotADrag() {
        #expect(press(CGPoint(x: dayWidth * 9, y: 100)) == nil)
    }

    @Test func minutesSnapToTheQuarterHour() {
        #expect(dragMinute(atY: hourHeight * 9 + 4, hourHeight: hourHeight) == 9 * 60)
        #expect(dragMinute(atY: hourHeight * 9 + 12, hourHeight: hourHeight) == 9 * 60 + 15)
    }
}

@Suite struct CalendarDragPreviewTests {
    private let standup = CalendarDragSubject(
        account: "acct", event: "evt", occurrenceStart: "",
        day: 1, startMinutes: 540, endMinutes: 600
    )

    private func move(dayDelta: Int, minuteDelta: Int) -> CalendarDragPreview {
        CalendarDragState(
            kind: .move, subject: standup,
            anchorDay: standup.day, anchorMinute: standup.startMinutes,
            day: standup.day + dayDelta, minute: standup.startMinutes + minuteDelta
        ).clamped(toColumns: daysInWeek).preview()
    }

    @Test func aMoveCarriesBothEdgesByTheSameAmount() {
        let preview = move(dayDelta: 1, minuteDelta: 30)
        #expect(preview.day == 2)
        #expect(preview.startMinutes == 570)
        #expect(preview.endMinutes == 630)
        #expect(preview.minutes == 60)  // the duration must survive a move exactly
    }

    @Test func aMoveIsClampedInsideItsOwnDay() {
        // What you see is what you get: an event dragged to the top of the screen stops at 00:00
        // rather than silently landing on the previous day. To change the day you drag sideways:
        // which is the one thing the preview can actually show.
        #expect(move(dayDelta: 0, minuteDelta: -900).startMinutes == 0)
        #expect(move(dayDelta: 0, minuteDelta: 900).endMinutes == dayMinutes)
    }

    @Test func aMoveIsClampedInsideTheWeek() {
        #expect(move(dayDelta: -5, minuteDelta: 0).day == 0)
        #expect(move(dayDelta: 12, minuteDelta: 0).day == daysInWeek - 1)
    }

    @Test func aResizeMovesOneEdgeAndCannotPassTheOther() {
        let start = CalendarDragState(
            kind: .resizeStart, subject: standup,
            anchorDay: standup.day, anchorMinute: 540, day: standup.day, minute: 540 + 600
        ).clamped(toColumns: daysInWeek).preview()
        #expect(start.endMinutes == 600)  // the end never moved
        #expect(start.startMinutes == 600 - 15)  // clamped to the minimum, not refused

        let end = CalendarDragState(
            kind: .resizeEnd, subject: standup,
            anchorDay: standup.day, anchorMinute: 600, day: standup.day, minute: 600 - 600
        ).clamped(toColumns: daysInWeek).preview()
        #expect(end.startMinutes == 540)  // the start never moved
        #expect(end.endMinutes == 540 + 15)
    }

    @Test func aPressThatNeverMovedDrawsAnHour() {
        let create = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 2, anchorMinute: 600, day: 2, minute: 600
        ).preview()
        #expect(create.startMinutes == 600)
        #expect(create.endMinutes == 660)
    }

    @Test func aPressThatWasDraggedDrawsWhatTheHandDescribed() {
        let create = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 2, anchorMinute: 600, day: 2, minute: 690
        ).preview()
        #expect(create.startMinutes == 600)
        #expect(create.endMinutes == 690)
    }

    @Test func aSlotDraggedUpwardsRunsFromThePointerToTheHourItBeganIn() {
        // Upwards the hand is setting the *start*. The end is the bottom of the band the press
        // landed in, 11:00, not the 10:00 the press happened to be on. See CalendarDragFeelTests.
        let create = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 2, anchorMinute: 600, day: 2, minute: 510
        ).preview()
        #expect(create.startMinutes == 510)
        #expect(create.endMinutes == 660)
    }

    @Test func aSlotStaysInTheColumnItBeganIn() {
        // Widening a slot across days is not a thing an event can be, so a sideways wobble while
        // drawing one out must not silently file it on Wednesday.
        let create = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 2, anchorMinute: 600, day: 5, minute: 690
        ).preview()
        #expect(create.day == 2)
    }

    @Test func aPressThatWentNowhereWritesNothing() {
        let still = CalendarDragState(
            kind: .move, subject: standup, anchorDay: 1, anchorMinute: 540, day: 1, minute: 540
        )
        #expect(!still.movesAnything)
        // ...but a create always does: the press itself was the request.
        let created = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 1, anchorMinute: 540, day: 1, minute: 540
        )
        #expect(created.movesAnything)
    }
}

@Suite struct CalendarDragArgsTests {
    private let once = CalendarDragSubject(
        account: "acct", event: "evt", occurrenceStart: "",
        day: 1, startMinutes: 540, endMinutes: 600
    )
    private let weekly = CalendarDragSubject(
        account: "acct", event: "evt", occurrenceStart: "2026-07-07T09:00:00",
        day: 1, startMinutes: 540, endMinutes: 600
    )

    @Test func aMoveSendsADeltaNotADestination() {
        let args = CalendarDragState(
            kind: .move, subject: once, anchorDay: 1, anchorMinute: 540, day: 2, minute: 570
        ).moveArgs(thisOccurrenceOnly: false)
        #expect(args?.edge == .whole)
        #expect(args?.days == 1)
        #expect(args?.minutes == 30)
    }

    @Test func eachResizeNamesItsOwnEdge() {
        let start = CalendarDragState(
            kind: .resizeStart, subject: once, anchorDay: 1, anchorMinute: 540, day: 1, minute: 525
        ).moveArgs(thisOccurrenceOnly: false)
        #expect(start?.edge == .start)

        let end = CalendarDragState(
            kind: .resizeEnd, subject: once, anchorDay: 1, anchorMinute: 600, day: 1, minute: 630
        ).moveArgs(thisOccurrenceOnly: false)
        #expect(end?.edge == .end)
    }

    @Test func aRepeatingEventIsAskedAboutAndAOneOffIsNot() {
        let repeating = CalendarDragState(
            kind: .move, subject: weekly, anchorDay: 1, anchorMinute: 540, day: 2, minute: 540
        )
        let oneOff = CalendarDragState(
            kind: .move, subject: once, anchorDay: 1, anchorMinute: 540, day: 2, minute: 540
        )
        #expect(repeating.asksAboutTheSeries)
        #expect(!oneOff.asksAboutTheSeries)
    }

    @Test func thisEventNamesTheOccurrenceAndAllEventsNamesNone() {
        let drag = CalendarDragState(
            kind: .move, subject: weekly, anchorDay: 1, anchorMinute: 540, day: 2, minute: 540
        )
        #expect(drag.moveArgs(thisOccurrenceOnly: true)?.occurrence == "2026-07-07T09:00:00")
        // The whole series is named by sending no occurrence at all.
        #expect(drag.moveArgs(thisOccurrenceOnly: false)?.occurrence == nil)
    }

    @Test func aOneOffNeverNamesAnOccurrenceWhateverItIsAsked() {
        let drag = CalendarDragState(
            kind: .move, subject: once, anchorDay: 1, anchorMinute: 540, day: 2, minute: 540
        )
        #expect(drag.moveArgs(thisOccurrenceOnly: true)?.occurrence == nil)
    }

    @Test func aCreateAsksForNoMoveAtAll() {
        let drag = CalendarDragState(
            kind: .create, subject: nil, anchorDay: 1, anchorMinute: 540, day: 1, minute: 600
        )
        #expect(drag.moveArgs(thisOccurrenceOnly: false) == nil)
    }
}

@Suite struct CalendarDragEditorTests {

    @Test func aDraggedSlotOpensTheEditorOnTheTimeThatWasDrawn() {
        // The "New event" button rounds to the next whole hour, because the user has said nothing
        // about when. A drag has said exactly when, rounding on top of it throws the gesture away.
        let cal = Calendar(identifier: .gregorian)
        var components = DateComponents()
        components.year = 2026
        components.month = 7
        components.day = 7
        components.hour = 10
        components.minute = 45
        let drawn = cal.date(from: components)!

        let editor = EventEditorState.create(
            default: nil, zone: "Europe/Amsterdam", now: drawn, minutes: 90, exact: true
        )
        #expect(editor.start == drawn)
        #expect(editor.end == drawn.addingTimeInterval(90 * 60))
    }

    @Test func theNewEventButtonStillRoundsToTheNextWholeHour() {
        let cal = Calendar(identifier: .gregorian)
        var components = DateComponents()
        components.year = 2026
        components.month = 7
        components.day = 7
        components.hour = 10
        components.minute = 15
        let now = cal.date(from: components)!

        let editor = EventEditorState.create(default: nil, zone: "Europe/Amsterdam", now: now)
        #expect(cal.component(.hour, from: editor.start) == 11)
        #expect(cal.component(.minute, from: editor.start) == 0)
    }
}
