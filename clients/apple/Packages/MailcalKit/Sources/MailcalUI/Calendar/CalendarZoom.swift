// Pinch-to-zoom: how much of the day, and how many of the week's days, the grid shows at once.
//
// The *horizon* (hours) and the *column count* (days) are persisted core settings, so all three
// clients open the same way. But neither an hour nor a day has a **size** until a client multiplies:
// the core's geometry is deliberately unit-free, so the zoom itself lives here.
//
// The rules are a plain type with no SwiftUI in it, so they can be tested without a two-finger
// gesture on a device. That matters: everything that can actually be *wrong* about a pinch is here:
// which way it runs, where it stops, and whether the content stays under the fingers.

import CoreGraphics
import Foundation
import MailcalBindings

// The same clamps the core enforces (mailcal-account: MIN/MAX_VISIBLE_HOURS), held here too so the
// live gesture is bounded as it happens rather than snapping back when the core rejects it.
let minVisibleHours: CGFloat = 4
let maxVisibleHours: CGFloat = 24

/// You can zoom in to a single day and out to the whole week. Never further: the week IS the page.
let minVisibleDays: CGFloat = 1
let maxVisibleDays = CGFloat(daysInWeek)

/// How much of the day, and how many of the week's days, are on screen.
///
/// Both are **fractional**. A pinch is continuous, and rounding mid-gesture would make the grid jump
/// between whole hours (or whole columns) instead of tracking the fingers, which is the difference
/// between "buttery" and "broken". Only the settled values are whole, and only those go back to the
/// core to be persisted.
struct CalendarZoom {
    private(set) var visibleHours: CGFloat
    private(set) var visibleDays: CGFloat

    init(visibleHours: Int, visibleDays: Int = 3) {
        self.visibleHours = min(max(CGFloat(visibleHours), minVisibleHours), maxVisibleHours)
        self.visibleDays = min(max(CGFloat(visibleDays), minVisibleDays), maxVisibleDays)
    }

    /// Applies one frame of a vertical pinch, returning **the factor the hour height actually grew
    /// by**, which is not the factor asked for once the zoom hits its clamp.
    ///
    /// The caller needs the real one: it corrects the scroll offset by exactly this, to keep the
    /// content under the fingers still. Correcting by the *requested* factor at the end of the range
    /// would drag the grid on every further frame of a pinch that has nowhere left to go.
    ///
    /// `zoom > 1` is fingers spreading apart, which must show *fewer* hours (zoom in), so it
    /// divides. Get that backwards and the grid zooms out when the user pinches in.
    @discardableResult
    mutating func pinchVertical(_ zoom: CGFloat) -> CGFloat {
        guard zoom > 0 else { return 1 }
        let before = visibleHours
        visibleHours = min(max(visibleHours / zoom, minVisibleHours), maxVisibleHours)
        return before / visibleHours
    }

    /// The same, for the day axis: spreading sideways shows fewer, wider days.
    @discardableResult
    mutating func pinchHorizontal(_ zoom: CGFloat) -> CGFloat {
        guard zoom > 0 else { return 1 }
        let before = visibleDays
        visibleDays = min(max(visibleDays / zoom, minVisibleDays), maxVisibleDays)
        return before / visibleDays
    }

    /// The whole-hour horizon to persist once the fingers lift.
    var settledHours: Int {
        Int(visibleHours.rounded()).clamped(to: Int(minVisibleHours)...Int(maxVisibleHours))
    }

    /// The whole-column count to persist once the fingers lift.
    var settledDays: Int {
        Int(visibleDays.rounded()).clamped(to: Int(minVisibleDays)...Int(maxVisibleDays))
    }

    /// Re-seeds the horizon (on load, or when the settings screen changes it).
    mutating func resetHours(_ hours: Int) {
        visibleHours = min(max(CGFloat(hours), minVisibleHours), maxVisibleHours)
    }

    /// Re-seeds the day axis (on load, or when a shape is picked from the menu).
    mutating func resetDays(_ days: Int) {
        visibleDays = min(max(CGFloat(days), minVisibleDays), maxVisibleDays)
    }

    /// Snaps the day axis to the **zoom level** the pinch settled on, once the fingers lift.
    ///
    /// A column count that is not a rung leaves part of the week hanging off the side of the screen,
    /// because the page always holds all seven days, and that overhang is a scroll competing with
    /// the gesture above it.
    ///
    /// It snaps to the settled *level's* columns and not to ``settledDays``, and the difference is
    /// the bug: a pinch outwards from the week lands on ~6.4 columns, which **rounds to 6** while the
    /// level it maps to is the whole week, of **7**. Settling on the rounded count would draw seven
    /// columns at one-sixth of the viewport each, the width and the mode disagreeing by a whole
    /// column, in the one view that is supposed to have no overhang at all.
    mutating func settleDays() {
        resetDays(modeForColumns(settledDays).columns)
    }

    /// How tall one hour is, given the height of the grid's viewport. The bridge from the core's
    /// unit-free geometry to points: every block's vertical offset and height is a multiple of it.
    func hourHeight(viewport: CGFloat) -> CGFloat { viewport / visibleHours }

    /// How wide one day column is. The horizontal twin.
    func dayWidth(viewport: CGFloat) -> CGFloat { viewport / visibleDays }
}

/// How far the day axis can be scrolled: the week's whole width, less the viewport it is seen
/// through. Zero when the zoom shows all seven days, the week is the page, so there is nowhere to go.
func calendarMaxDayOffset(dayWidth: CGFloat, dayCount: Int, viewportWidth: CGFloat) -> CGFloat {
    max(dayWidth * CGFloat(dayCount) - viewportWidth, 0)
}

/// The hour axis's twin: a whole day of content, less the height of the grid it is seen through.
func calendarMaxHourOffset(hourHeight: CGFloat, gridHeight: CGFloat) -> CGFloat {
    max(hourHeight * CGFloat(calendarHours) - gridHeight, 0)
}

/// The scroll offset that keeps whatever was under `focus` exactly under `focus`, after the content
/// has been scaled by `factor`.
///
/// The content point under the fingers sits at `scroll + focus` points along the content. Scaling
/// moves it to `(scroll + focus) * factor`; putting it back under the same finger means scrolling to
/// that, less the finger's own offset in the viewport.
///
/// Without this the offset stays fixed in **points** while the scale changes, so the same offset maps
/// to a different time, and the grid slides out from under the user's fingers, appearing to zoom
/// about the top of the day rather than about their hand.
func focalPreservingScroll(scroll: CGFloat, focus: CGFloat, factor: CGFloat) -> CGFloat {
    max((scroll + focus) * factor - focus, 0)
}

extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
