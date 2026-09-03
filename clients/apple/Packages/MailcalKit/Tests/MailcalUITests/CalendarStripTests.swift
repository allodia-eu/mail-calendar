// The strip: the grid's horizontal axis, and the rules a scroll across a week boundary has to keep.
//
// These pin the two things that made the week a wall on Apple until now, and the one thing that
// replaces it. A scroll runs straight through a boundary; re-anchoring moves the grid by exactly
// nothing; and whatever ends a gesture, the strip comes to rest on a **day**, never on a week.
//
// A week-sized landing needs a threshold ("did it travel far enough?"), and a threshold discards
// what the user did: it is what rubber-banded the Windows grid home six times in thirteen seconds
// on a slow trackpad pan (docs/calendar.md §6). A day boundary needs no threshold and no judgement.

import CoreGraphics
import Testing

@testable import MailcalUI

/// A day column, in points, and the week it belongs to. The viewport shows three of them.
private let dayWidth: CGFloat = 100
private let weekWidth = dayWidth * CGFloat(daysInWeek)
private let threeDayViewport = dayWidth * 3

@Suite struct CalendarStripTests {

    @Test func aScrollRunsStraightThroughAWeekBoundary() {
        // The whole point. Scrolling right from Friday of week 0 reaches Monday of week 1 without
        // anything turning, stopping or springing back: the days are one strip.
        var strip = CalendarStrip()
        strip.frame(week: 0, column: 4)
        strip.pan(-dayWidth * 3, dayWidth: dayWidth)

        #expect(strip.anchorWeek == 1)
        // Day 7 of the strip is week 1's first day, and it is now at the left edge.
        #expect(abs(strip.weeks - 1) < 0.0001)
    }

    @Test func reAnchoringMovesTheGridByExactlyNothing() {
        // The strip's only discontinuity, and one nothing can see: the anchor changes, and the week
        // it left behind is drawn at the same pixel it was a moment ago.
        var strip = CalendarStrip()
        strip.frame(week: 0, column: 6)
        let before = strip.origin(ofWeek: 0, dayWidth: dayWidth)
        #expect(strip.anchorWeek == 0)

        // A hair past the boundary: the anchor flips to week 1.
        strip.pan(-dayWidth, dayWidth: dayWidth)
        #expect(strip.anchorWeek == 1)
        #expect(strip.origin(ofWeek: 0, dayWidth: dayWidth) == before - dayWidth)
        // And week 1 begins exactly where week 0 ends. No gutter, no seam.
        #expect(
            abs(
                strip.origin(ofWeek: 1, dayWidth: dayWidth)
                    - (strip.origin(ofWeek: 0, dayWidth: dayWidth) + weekWidth)
            ) < 0.0001
        )
    }

    @Test func aSeamPutsTwoWeeksOnScreenAndABoundaryPutsOne() {
        var strip = CalendarStrip()
        // Whole-week zoom, resting on a boundary: seven columns fill the viewport exactly, so there
        // is nothing of the next week to draw.
        strip.frame(week: 3, column: 0)
        #expect(strip.visibleWeeks(dayViewport: weekWidth, dayWidth: dayWidth) == [3])

        // One column along, and the week after it is on screen.
        strip.frame(week: 3, column: 1)
        #expect(strip.visibleWeeks(dayViewport: weekWidth, dayWidth: dayWidth) == [3, 4])

        // A narrow zoom that fits inside one week sees only that week, wherever it is parked.
        strip.frame(week: 3, column: 2)
        #expect(strip.visibleWeeks(dayViewport: threeDayViewport, dayWidth: dayWidth) == [3])
        // Until it straddles the boundary.
        strip.frame(week: 3, column: 5)
        #expect(strip.visibleWeeks(dayViewport: threeDayViewport, dayWidth: dayWidth) == [3, 4])
    }

    @Test func theStripComesToRestOnADayNotOnAWeek() {
        // A third of a column past Wednesday lands back on Wednesday; two thirds lands on Thursday.
        // Neither one turns a page, and neither is judged against a threshold.
        var strip = CalendarStrip()
        strip.pan(-(dayWidth * 3 + dayWidth / 3), dayWidth: dayWidth)
        #expect(abs(strip.nearestDay - 3.0 / CGFloat(daysInWeek)) < 0.0001)

        strip.frame(week: 0, column: 3)
        strip.pan(-(dayWidth * 2 / 3), dayWidth: dayWidth)
        #expect(abs(strip.nearestDay - 4.0 / CGFloat(daysInWeek)) < 0.0001)
    }

    @Test func aLandingNeverTravelsMoreThanHalfAColumn() {
        // The property that makes the landing read as settling rather than as being overruled, at
        // every zoom and from every position, including across a boundary.
        var strip = CalendarStrip()
        for step in 0..<64 {
            strip.weeks = CGFloat(step) * 0.031 - 1
            let travel = abs(strip.nearestDay - strip.weeks) * weekWidth
            #expect(travel <= dayWidth / 2 + 0.0001, "landed \(travel)pt away")
        }
    }

    @Test func aLandingOnASeamRestsBetweenTwoWeeks() {
        // Sunday beside Monday is a resting place, which is exactly what a page could never be.
        var strip = CalendarStrip()
        strip.frame(week: 0, column: 6)
        strip.pan(-4, dayWidth: dayWidth)
        strip.weeks = strip.nearestDay

        #expect(strip.anchorWeek == 0)
        #expect(abs(strip.weeks - 6.0 / CGFloat(daysInWeek)) < 0.0001)
        #expect(strip.visibleWeeks(dayViewport: threeDayViewport, dayWidth: dayWidth) == [0, 1])
    }

    @Test func aReversedPanReversesImmediately() {
        // No banked landing to disagree with, so a hand that changes its mind is simply obeyed.
        var strip = CalendarStrip()
        strip.pan(-dayWidth * 2, dayWidth: dayWidth)
        let forward = strip.weeks
        strip.pan(dayWidth * 3, dayWidth: dayWidth)
        #expect(strip.weeks < forward)
        #expect(abs(strip.weeks - (-1.0 / CGFloat(daysInWeek))) < 0.0001)
    }

    @Test func theStripRunsBackwardsPastItsOriginWeek() {
        // Nothing bounds this axis, so last week is a scroll away in both directions.
        var strip = CalendarStrip()
        strip.pan(dayWidth * 10, dayWidth: dayWidth)
        #expect(strip.anchorWeek == -2)
        #expect(strip.visibleWeeks(dayViewport: threeDayViewport, dayWidth: dayWidth) == [-2])
        #expect(strip.origin(ofWeek: -2, dayWidth: dayWidth) < 0)
        // Nine days back is a seam of its own, in the direction where a truncating division would
        // have quietly filed the left edge in the week after the one it is in.
        strip.pan(-dayWidth, dayWidth: dayWidth)
        #expect(strip.visibleWeeks(dayViewport: threeDayViewport, dayWidth: dayWidth) == [-2, -1])
    }

    @Test func aPressIsResolvedAgainstTheWeekItLandedIn() {
        // The seam again, from the pointer's side: a press on the right half of a straddled viewport
        // belongs to the *next* week's page, at a column inside it.
        var strip = CalendarStrip()
        strip.frame(week: 2, column: 6)

        let sunday = strip.location(atX: dayWidth / 2, dayWidth: dayWidth)
        #expect(sunday.week == 2)
        #expect(sunday.column == 6)

        let monday = strip.location(atX: dayWidth * 1.5, dayWidth: dayWidth)
        #expect(monday.week == 3)
        #expect(monday.column == 0)
    }

    @Test func aPressBeforeTheOriginWeekStillLandsInsideAWeek() {
        // Integer division truncates towards zero, so a negative day index is where a column comes
        // back as -1 and the press is filed against the wrong page.
        var strip = CalendarStrip()
        strip.frame(week: 0, column: 0)

        let back = strip.location(atX: -dayWidth * 0.5, dayWidth: dayWidth)
        #expect(back.week == -1)
        #expect(back.column == 6)
    }

    @Test func aPinchKeepsTheDayUnderTheFingersUnderTheFingers() {
        // Zooming in about a point two columns into the viewport leaves that same day there.
        var strip = CalendarStrip()
        strip.frame(week: 1, column: 2)
        let focus = dayWidth * 2
        let dayUnderFingers = strip.weeks * CGFloat(daysInWeek) + focus / dayWidth

        strip.pinch(factor: 2, focus: focus, dayWidthBefore: dayWidth, dayWidthAfter: dayWidth * 2)

        let after = strip.weeks * CGFloat(daysInWeek) + focus / (dayWidth * 2)
        #expect(abs(after - dayUnderFingers) < 0.0001)
    }

    @Test func aPinchAtAWeeksFirstDayAnchorsOnTheWeekBefore() {
        // The bound this axis deliberately does not have. Zooming out at the start of a week pulls
        // earlier days into view, so the anchor moves back a week rather than the days creeping.
        var strip = CalendarStrip()
        strip.frame(week: 4, column: 0)
        strip.pinch(
            factor: 0.5, focus: dayWidth * 2, dayWidthBefore: dayWidth, dayWidthAfter: dayWidth / 2
        )

        #expect(strip.weeks < 4)
        #expect(strip.anchorWeek == 3)
    }

    @Test func theWideShapesFrameOnTheWeekAndTheNarrowOnesOnToday() {
        // Not a clamp, an answer. "Work week" means Monday to Friday or it means nothing, and a
        // whole week framed on today opens on a grid that begins on Tuesday.
        #expect(calendarFramingColumn(mode: .week, todayColumn: 1) == 0)
        #expect(calendarFramingColumn(mode: .workWeek, todayColumn: 1) == 0)
        #expect(calendarFramingColumn(mode: .day, todayColumn: 1) == 1)
        #expect(calendarFramingColumn(mode: .threeDay, todayColumn: 1) == 1)
        // A Sunday at the 3-day zoom means Sunday to Tuesday, running across the boundary, which is
        // what the rule always described and only a strip can draw.
        #expect(calendarFramingColumn(mode: .threeDay, todayColumn: 6) == 6)
        // No today on the page (a week the user paged away to): frame on its first day.
        #expect(calendarFramingColumn(mode: .day, todayColumn: nil) == 0)
    }

    @Test func onlyTheWeekHoldingTodayIsFramedOnToday() {
        // The regression this exists for: framing is applied on a **seat**, and one of those seats
        // is a shape picked from the menu, which re-origins on the week the user is *reading*, not
        // on today's. Offer today's column there and switching Week to Day while browsing next month
        // teleports you home, which is the one thing a shape change must not do.
        #expect(calendarTodayColumn(daysFromWeekStart: 0) == 0)
        #expect(calendarTodayColumn(daysFromWeekStart: 6) == 6)
        // A week ahead, and a week behind: neither holds today, so neither has a column for it.
        #expect(calendarTodayColumn(daysFromWeekStart: 7) == nil)
        #expect(calendarTodayColumn(daysFromWeekStart: -1) == nil)
        #expect(calendarTodayColumn(daysFromWeekStart: 34) == nil)
        #expect(calendarTodayColumn(daysFromWeekStart: nil) == nil)
        // And the two together: a week that does not hold today opens on its first day, at the zoom
        // that would otherwise have opened on today's column.
        #expect(
            calendarFramingColumn(
                mode: .day, todayColumn: calendarTodayColumn(daysFromWeekStart: 30)
            ) == 0
        )
    }
}
