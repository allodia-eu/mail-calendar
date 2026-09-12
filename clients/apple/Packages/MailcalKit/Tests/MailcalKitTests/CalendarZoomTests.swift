// Pinch-to-zoom: the rules, without a two-finger gesture.
//
// The gesture itself has to be tried on a device, but everything that can actually be *wrong* about it
// is here, which way it runs, where it stops, which axis it belongs to, and (the bug that made the
// first Android build feel broken) whether the content stays under the fingers.

import CoreGraphics
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct CalendarZoomTests {

    @Test func spreadingYourFingersShowsLessOfTheDayNotMore() {
        // The direction is the whole feel of the gesture. Spreading (zoom > 1) means "closer", which
        // means FEWER hours on screen. Get it backwards and the grid zooms out when the user pinches
        // in, instantly, obviously broken.
        var zoom = CalendarZoom(visibleHours: 12)
        zoom.pinchVertical(2)
        #expect(abs(zoom.visibleHours - 6) < 0.01)

        zoom.pinchVertical(0.5)
        #expect(abs(zoom.visibleHours - 12) < 0.01)
    }

    @Test func theHorizonStopsAtTheCoresLimits() {
        // A pinch runs off the end of its own gesture constantly, fingers keep moving after the grid
        // has nothing left to give. It must stop, not invert or divide by nothing.
        var zoom = CalendarZoom(visibleHours: 12)
        for _ in 0..<20 { zoom.pinchVertical(2) }
        #expect(abs(zoom.visibleHours - minVisibleHours) < 0.01)

        for _ in 0..<40 { zoom.pinchVertical(0.5) }
        #expect(abs(zoom.visibleHours - maxVisibleHours) < 0.01)
    }

    @Test func aZoomReportsTheFactorItActuallyAppliedNotTheOneItWasAskedFor() {
        // The caller corrects the scroll by this, to keep the content under the fingers still. At the
        // clamp the hour height did NOT change, so the factor must be exactly 1, or the grid would be
        // dragged on every further frame of a pinch that has nowhere left to go. It is also what lets
        // a diagonal pinch that runs out of hours keep zooming days, instead of one axis dragging the
        // other to a halt.
        var zoom = CalendarZoom(visibleHours: 12)
        #expect(abs(zoom.pinchVertical(2) - 2) < 0.01)  // 12h -> 6h: an hour doubled in height

        for _ in 0..<20 { zoom.pinchVertical(2) }  // pinned at the minimum horizon
        #expect(abs(zoom.pinchVertical(2) - 1) < 0.001)
    }

    @Test func aGarbageScaleFactorLeavesTheZoomAlone() {
        var zoom = CalendarZoom(visibleHours: 12)
        #expect(abs(zoom.pinchVertical(0) - 1) < 0.001)
        #expect(abs(zoom.pinchVertical(-3) - 1) < 0.001)
        #expect(abs(zoom.visibleHours - 12) < 0.01)
    }

    @Test func theContentUnderYourFingersStaysUnderYourFingers() {
        // THE bug. Without this the scroll offset stays fixed in POINTS while the hour height changes,
        // so the same offset maps to an earlier and earlier time, the grid slides out from under the
        // fingers and the zoom appears anchored to the top of the day rather than to the user's hand.
        //
        // Scrolled 1000pt down, fingers 500pt into the viewport: the content point under them is
        // 1500pt down the day. Double the scale and it sits at 3000, so the scroll must become
        // 3000 - 500 = 2500 to leave it exactly where it was.
        #expect(abs(focalPreservingScroll(scroll: 1000, focus: 500, factor: 2) - 2500) < 0.01)
        // Zooming out by the same factor puts it back.
        #expect(abs(focalPreservingScroll(scroll: 2500, focus: 500, factor: 0.5) - 1000) < 0.01)
        // A zoom that changed nothing must not move the scroll at all.
        #expect(abs(focalPreservingScroll(scroll: 1234, focus: 400, factor: 1) - 1234) < 0.01)
        // The formula itself is unbounded, and the caller is what bounds it: the hour axis clamps to
        // the top of the day, the day axis to nothing at all, because the strip is endless and a
        // bound there is what makes a pinch at the end of a week creep the days sideways.
        #expect(focalPreservingScroll(scroll: 0, focus: 10, factor: 0.2) < 0)
        // The degenerate case that proves it is anchored and not merely scaled.
        #expect(abs(focalPreservingScroll(scroll: 0, focus: 0, factor: 3)) < 0.01)
    }

    @Test func anAxisTheFingersAreNotSpreadAlongReportsNoChange() {
        // What keeps the axes independent WITHOUT forbidding a diagonal. In a purely horizontal pinch
        // the fingers sit at almost the same height, so the vertical spread is a few noisy points:
        // dividing by it would produce a wild factor and lurch the hours about while the user was only
        // asking for more days.
        #expect(abs(axisScale(before: 3, after: 9) - 1) < 0.001)
        #expect(abs(axisScale(before: 200, after: 4) - 1) < 0.001)
        // Genuinely spread along the axis: a real scale.
        #expect(abs(axisScale(before: 100, after: 200) - 2) < 0.001)
    }

    @Test func theNoiseFloorIsMeasuredAgainstTheDeviceNotInPoints() {
        // A trackpad reports touches in its OWN units, not in points, so the phone's 48pt floor means
        // nothing there, and hard-coding it would either swallow every real macOS pinch or let the
        // noise through, depending on the Mac. The floor is a fraction of the trackpad instead.
        //
        // A 1000-unit-wide trackpad: a tenth of it is 100 units, so a 60-unit spread is noise and a
        // 300-unit spread is a gesture. The identical numbers on a phone would say the opposite.
        let floor = 1000 * 0.1
        #expect(abs(axisScale(before: 60, after: 90, minimum: floor) - 1) < 0.001)
        #expect(abs(axisScale(before: 300, after: 600, minimum: floor) - 2) < 0.001)
        // ...and the same 300→600 on the phone's floor is a real gesture too, but 60→90 is *not* noise
        // by the phone's reckoning. Same inputs, different verdict, which is exactly why it is passed
        // in rather than assumed.
        #expect(abs(axisScale(before: 60, after: 90) - 1.5) < 0.001)
    }

    @Test func aDiagonalPinchZoomsBothAxesEachByItsOwnSpread() {
        // The gesture that makes the grid feel alive: drag your fingers apart at an angle and the
        // hours and the days both stretch, each by how far the fingers actually travelled ALONG that
        // axis, not by some blended average of the two.
        //
        // 100pt apart horizontally and 200pt vertically, pulled to 200 and 300: the day axis scaled
        // 2.0, the hour axis only 1.5. Neither may borrow from the other.
        let xScale = axisScale(before: 100, after: 200)
        let yScale = axisScale(before: 200, after: 300)
        #expect(abs(xScale - 2.0) < 0.001)
        #expect(abs(yScale - 1.5) < 0.001)

        var zoom = CalendarZoom(visibleHours: 12, visibleDays: 6)
        zoom.pinchVertical(yScale)
        zoom.pinchHorizontal(xScale)
        #expect(abs(zoom.visibleHours - 8) < 0.01)  // 12 / 1.5
        #expect(abs(zoom.visibleDays - 3) < 0.01)  // 6 / 2.0
    }

    @Test func aDiagonalPinchThatRunsOutOfHoursKeepsZoomingDays() {
        // Each axis clamps on its own. If they were locked together, or if one dragged the other:
        // hitting the 4-hour floor would freeze the day axis mid-gesture, and the pinch would die in
        // the user's hand.
        var zoom = CalendarZoom(visibleHours: 5, visibleDays: 7)
        for _ in 0..<10 {
            zoom.pinchVertical(2)
            zoom.pinchHorizontal(1.2)
        }
        #expect(abs(zoom.visibleHours - minVisibleHours) < 0.01, "the hours are pinned at their floor")
        #expect(zoom.visibleDays < 7, "but the days kept going")
    }

    @Test func theWeekIsTheBoundaryAZoomCannotCross() {
        // You can zoom in to a single day and out to the whole week. Never further: the week IS the
        // page, and beyond it you swipe rather than zoom.
        var zoom = CalendarZoom(visibleHours: 12, visibleDays: 3)
        for _ in 0..<20 { zoom.pinchHorizontal(2) }
        #expect(abs(zoom.visibleDays - minVisibleDays) < 0.01)
        #expect(zoom.settledDays == 1)

        for _ in 0..<40 { zoom.pinchHorizontal(0.5) }
        #expect(abs(zoom.visibleDays - maxVisibleDays) < 0.01)
        #expect(zoom.settledDays == daysInWeek)
    }

    @Test func bothAxesStayFractionalMidPinch() {
        // Rounding while the fingers are still moving would make the grid stutter between whole hours
        // and whole columns instead of tracking them. Only the settled values are whole.
        var zoom = CalendarZoom(visibleHours: 12, visibleDays: 7)
        zoom.pinchHorizontal(1.2)
        zoom.pinchVertical(1.1)
        #expect(zoom.visibleDays.truncatingRemainder(dividingBy: 1) != 0)
        #expect(zoom.visibleHours.truncatingRemainder(dividingBy: 1) != 0)
    }

    @Test func theZoomIsWhatDecidesHowBigAnHourAndADayAre() {
        // The bridge from the core's unit-free geometry to points. "12 hours, 3 days" must mean the
        // same span on a phone and on a tablet, the cells just get bigger.
        var zoom = CalendarZoom(visibleHours: 12, visibleDays: 3)
        #expect(abs(zoom.hourHeight(viewport: 600) - 50) < 0.01)
        #expect(abs(zoom.dayWidth(viewport: 360) - 120) < 0.01)

        zoom.pinchVertical(2)
        #expect(abs(zoom.hourHeight(viewport: 600) - 100) < 0.01)
    }

    @Test func aPersistedValueOutOfRangeIsPulledBackIn() {
        #expect(abs(CalendarZoom(visibleHours: 0).visibleHours - minVisibleHours) < 0.01)
        #expect(abs(CalendarZoom(visibleHours: 99).visibleHours - maxVisibleHours) < 0.01)
        #expect(abs(CalendarZoom(visibleHours: 12, visibleDays: 0).visibleDays - minVisibleDays) < 0.01)
        #expect(abs(CalendarZoom(visibleHours: 12, visibleDays: 99).visibleDays - maxVisibleDays) < 0.01)
    }

    @Test func aSettledPinchLandsOnARungSoTheWeekFillsTheViewportExactly() {
        // The bug that made a swipe stick. The page always holds all seven days, at a width of
        // viewport/visibleDays, so a count that is not a rung leaves part of the week hanging off
        // the screen, and that overhang is a scroll competing with the gesture above it.
        //
        // The subtlety is where it snaps TO. A pinch outwards from the week lands on ~6.4 columns,
        // which ROUNDS to 6, while the zoom level it maps to is the whole week, of 7. Settling on
        // the rounded number would draw seven columns at one-sixth of the viewport each: the width
        // and the mode disagreeing by a whole column, in the one view meant to have no overhang.
        var zoom = CalendarZoom(visibleHours: 12, visibleDays: 7)
        zoom.pinchHorizontal(1.1)
        #expect(zoom.settledDays == 6)
        #expect(modeForColumns(zoom.settledDays) == .week)

        zoom.settleDays()
        #expect(zoom.visibleDays == 7, "it must settle on the LEVEL's columns, not on the rounded count")

        let viewport: CGFloat = 700
        #expect(
            abs(zoom.dayWidth(viewport: viewport) * CGFloat(daysInWeek) - viewport) < 0.01,
            "the week must fill the viewport exactly, or the nested scroll eats the swipe"
        )
    }

    @Test func theCalendarReopensInTheShapeItWasLeftIn() {
        // The layout is a CORE setting, so it survives the app closing, and the phone and the Mac
        // cannot end up with different ideas of what "the calendar" looks like.
        #expect(CalendarMode.threeDay.layout == .threeDay)
        #expect(CalendarLayout.threeDay.mode == .threeDay)
        for mode in CalendarMode.allCases {
            #expect(mode.layout.mode == mode, "\(mode) must round-trip through the persisted setting")
        }
        // The month and the agenda have no columns of their own, but the grid behind them is seeded
        // with the whole week rather than with zero, which would divide the viewport by nothing.
        #expect(CalendarMode.month.gridColumns == daysInWeek)
        #expect(CalendarMode.agenda.gridColumns == daysInWeek)
        #expect(CalendarMode.threeDay.gridColumns == 3)
    }

    // MARK: - The offsets have to survive the content changing size

    @Test func shrinkingTheViewportLeavesTheGridScrolledPastItsOwnContent() {
        // The regression, reproduced on macOS 0.13.0 by dragging the window smaller: the grid was
        // left showing the gutter, the all-day band and nothing else, it reads as a calendar that
        // failed to load, and only a scroll gesture (which clamps) brought it back.
        //
        // Every clamp in CalendarGridView used to live inside a gesture, so nothing re-checked the
        // offsets when the *content* resized under them.
        let zoom = CalendarZoom(visibleHours: 12, visibleDays: 7)

        // Settled two-thirds down a tall window.
        let tallGrid: CGFloat = 900
        let tallHour = zoom.hourHeight(viewport: tallGrid)
        var hourOffset = calendarMaxHourOffset(hourHeight: tallHour, gridHeight: tallGrid) * 0.66
        #expect(hourOffset > 0, "the fixture has to be scrolled, or it proves nothing")

        // The window is dragged shorter. The hour height shrinks with it, so there is far less
        // content, and the offset that was valid a moment ago now points past the end of it.
        let shortGrid: CGFloat = 300
        let shortLimit = calendarMaxHourOffset(
            hourHeight: zoom.hourHeight(viewport: shortGrid), gridHeight: shortGrid
        )
        #expect(hourOffset > shortLimit, "unclamped, the grid is scrolled off its own content")

        hourOffset = hourOffset.clamped(to: 0...shortLimit)
        #expect(hourOffset == shortLimit)
    }

    @Test func aZoomArrivingAfterTheFirstFrameCannotStrandTheDayAxis() {
        // The bug this replaces, and why it cannot come back. The horizon and the column count are
        // CORE settings, so a client can render, and frame itself on today, before they arrive. When
        // the day axis was an offset in POINTS bounded by the week's width, that framing was measured
        // against the wrong geometry: recentring on Friday at three columns parked it 1.33 viewports
        // along, re-seeding to the whole week made the content exactly one viewport wide, and the
        // clamp then dragged every day off the left edge. The grid came up blank, which reads as a
        // rendering crash and is really a stale offset.
        //
        // The strip is measured in **weeks**, and a week is a week at every zoom, so the same
        // sequence leaves it on the day it was framed on and there is nothing left to strand.
        let viewport: CGFloat = 700
        var narrow = CalendarZoom(visibleHours: 12, visibleDays: 3)

        var strip = CalendarStrip()
        strip.frame(week: 0, column: 4)  // Friday, on a Monday-start week
        let framed = strip.weeks

        narrow.resetDays(daysInWeek)
        #expect(strip.weeks == framed, "the seeded zoom moved the strip")
        // And Friday is still the column against the grid's left edge, at the new width.
        #expect(
            strip.origin(ofWeek: 0, dayWidth: narrow.dayWidth(viewport: viewport))
                == -4 * narrow.dayWidth(viewport: viewport)
        )
    }
}
