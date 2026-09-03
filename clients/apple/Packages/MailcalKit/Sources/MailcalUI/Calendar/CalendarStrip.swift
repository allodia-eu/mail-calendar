// The grid's horizontal axis: one continuous strip of days, not a stack of week pages.
//
// The days are laid end to end with the hour ruler pinned beside them (CalendarGridView), so a grid
// showing Wednesday to Tuesday across a week boundary is a coherent frame rather than a half-turned
// page. That is what lets a two-finger scroll run straight through a week boundary instead of being
// resolved onto one, which is the rule docs/calendar.md §6 was written from.
//
// The strip is measured in **weeks**, never in pixels. A pixel means something different at every
// zoom, so a landing that outlived a pinch would arrive somewhere else entirely, and an offset
// counted in pixels from some far-off origin bleeds float precision as the user scrolls. Whole
// values are week boundaries; sevenths of one are days, and a day is the one place the strip comes
// to rest.
//
// No SwiftUI here, so every rule below is testable without a viewport (docs/calendar.md §11).

import CoreGraphics
import Foundation

/// Where the grid is along the strip, and what a gesture does to it.
struct CalendarStrip: Equatable, Sendable {
    /// The strip's left edge, in weeks from the pager's origin week.
    var weeks: CGFloat

    init(weeks: CGFloat = 0) {
        self.weeks = weeks
    }

    /// The week the grid pulls its page from.
    ///
    /// Bookkeeping, not a position: the user is no more "on" a week than a scrolled list is "on" a
    /// row. It changes when the left edge crosses a boundary, which moves the grid by exactly
    /// nothing, because `origin(ofWeek:)` measures from the same number.
    var anchorWeek: Int { Int(weeks.rounded(.down)) }

    /// Where week `index`'s first day sits, in points from the **day viewport's** left edge, past
    /// the pinned hour ruler. Negative for a week the left edge has already run into.
    func origin(ofWeek index: Int, dayWidth: CGFloat) -> CGFloat {
        (CGFloat(index) - weeks) * dayWidth * CGFloat(daysInWeek)
    }

    /// Every week with a column on screen, left to right.
    ///
    /// Two at a seam, one at rest on a week boundary in whole-week zoom, and more only if a viewport
    /// is ever wider than a week. Half a point of slack keeps a week that ends exactly at the right
    /// edge from adding a neighbour nobody can see.
    func visibleWeeks(dayViewport: CGFloat, dayWidth: CGFloat) -> [Int] {
        let week = dayWidth * CGFloat(daysInWeek)
        guard week > 0, dayViewport > 0 else { return [anchorWeek] }
        let last = Int((weeks + (dayViewport - 0.5) / week).rounded(.down))
        return Array(anchorWeek...max(last, anchorWeek))
    }

    /// Pans by a pointer's movement: `dx` is how far the content travelled with the hand, so a
    /// finger moving left brings on later days.
    ///
    /// One axis, one line. There is no day-scroll-then-page hand-off, because there is nothing to
    /// hand off to, and reversing mid-gesture needs no special case: the strip goes back the way it
    /// came.
    mutating func pan(_ dx: CGFloat, dayWidth: CGFloat) {
        let week = dayWidth * CGFloat(daysInWeek)
        guard week > 0 else { return }
        weeks -= dx / week
    }

    /// The nearest day boundary, the one place the strip rests.
    ///
    /// A day, at every zoom and for every input. It is the smallest unit that puts a column edge
    /// against the grid's left edge, so it is the least the grid can move and still look deliberate,
    /// and a landing at most half a column away reads as settling rather than as being overruled. A
    /// week-sized landing would drag the days by up to three and a half of them the user never asked
    /// to move, and would need a threshold, which is what rubber-bands a slow pan home.
    var nearestDay: CGFloat {
        (weeks * CGFloat(daysInWeek)).rounded() / CGFloat(daysInWeek)
    }

    /// Frames the grid on one column of one week: how it opens, and where "back to today" lands.
    mutating func frame(week: Int, column: Int) {
        weeks = CGFloat(week) + CGFloat(column) / CGFloat(daysInWeek)
    }

    /// The week and column a **day viewport** x falls in, for hit-testing a pointer.
    ///
    /// The column is always inside its week, so a caller can hand the press to that week's own page
    /// without knowing the strip is continuous.
    func location(atX x: CGFloat, dayWidth: CGFloat) -> (week: Int, column: Int) {
        guard dayWidth > 0 else { return (anchorWeek, 0) }
        let day = Int((weeks * CGFloat(daysInWeek) + x / dayWidth).rounded(.down))
        let week = Int((CGFloat(day) / CGFloat(daysInWeek)).rounded(.down))
        return (week, day - week * daysInWeek)
    }

    /// The strip position that keeps whatever was under `focus` there after the columns have been
    /// scaled by `factor`, `focus` being a point in the day viewport.
    ///
    /// The day axis is corrected with **no bound of its own**: the strip is endless, so a pinch that
    /// pulls the focus day back past the anchor week's first day simply anchors on the week before
    /// it. A bound here is what made a pinch at the end of a week creep the days sideways.
    mutating func pinch(factor: CGFloat, focus: CGFloat, dayWidthBefore: CGFloat, dayWidthAfter: CGFloat) {
        let before = dayWidthBefore * CGFloat(daysInWeek)
        let after = dayWidthAfter * CGFloat(daysInWeek)
        guard before > 0, after > 0 else { return }
        weeks = focalPreservingScroll(scroll: weeks * before, focus: focus, factor: factor) / after
    }
}

/// How long a coast of `points` takes to run out.
///
/// Scaled by the distance rather than fixed, because the same duration for both jobs this serves is
/// wrong twice: a half-column landing over 0.5s reads as the grid hesitating, and a hard flick's
/// several hundred points over 0.2s reads as a teleport. The bounds are hand-tuned on a trackpad and
/// a phone, and the curve is `easeOut`, which is what a scroll's own deceleration looks like.
func calendarCoastDuration(points: CGFloat) -> Double {
    min(0.6, max(0.18, Double(abs(points)) / 1400))
}

/// The column the grid frames on when it opens, jumps home, or is given a shape from the menu.
///
/// A shared product decision, held by one helper on each client (`framingColumn` on Android,
/// `FramingColumn` on Windows) rather than re-derived per view: the two wide shapes frame on the
/// week's first day, the two narrow ones on today.
///
/// The wide shapes return `0` outright rather than leaving it to a clamp. A clamp is a rule holding
/// because of a bound somewhere else, and this axis no longer has one: framing the whole week on
/// today would open a grid that *begins* on Tuesday, a Tuesday under the first heading, which is the
/// misalignment week-start exists to prevent (docs/calendar.md §3).
/// Today's column in the week being framed, or `nil` when today is not in that week.
///
/// `days` is the distance from the framed week's first day to today, so only `0..<7` is a column at
/// all. The rest is the answer to a different question, and handing it to `calendarFramingColumn`
/// would frame a week the user is browsing on a column belonging to today's: switch Week to Day
/// while reading next month and the grid teleports home.
func calendarTodayColumn(daysFromWeekStart days: Int?) -> Int? {
    days.flatMap { (0..<daysInWeek).contains($0) ? $0 : nil }
}

func calendarFramingColumn(mode: CalendarMode, todayColumn: Int?) -> Int {
    switch mode {
    case .week, .workWeek: return 0
    default: return todayColumn ?? 0
    }
}
