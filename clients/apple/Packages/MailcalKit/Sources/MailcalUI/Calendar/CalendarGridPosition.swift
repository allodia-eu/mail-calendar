// Where the time grid is: the strip, the hour offset and the zoom, which every frame of a scroll, a
// landing or a pinch moves.
//
// Held in an observable object rather than in the screen's own `@State`, so that a frame of motion
// re-evaluates only the views that read these, the grid itself, and never the screen around it,
// whose toolbar AppKit would lay out again on every frame (docs/calendar.md §7). The screen reads
// the two facts it draws from them (which week is anchored, whether today is on screen), published
// only when they change.

import SwiftUI

@MainActor
@Observable
final class CalendarGridPosition {
    /// The days, in weeks from the pager's origin.
    var strip = CalendarStrip() { didSet { publish() } }
    var hourOffset: CGFloat = 0
    var zoom: CalendarZoom { didSet { publish() } }
    /// Whether a hand has moved the grid: a scroll, a flick, a pinch or a chevron.
    ///
    /// Until one has, the grid is the app's to frame, and every layout pass re-frames it on today and
    /// now. After one has, nothing but a deliberate seat (a shape from the menu, "back to today") may
    /// move it again. Read only from actions, never from a body.
    @ObservationIgnored var held = false

    /// Today's column, counted from the origin week's first day, as the screen last worked it out.
    @ObservationIgnored var todayColumn: Int? { didSet { publish() } }

    /// The week the strip's left edge is in.
    private(set) var anchorWeek = 0
    /// Whether today's column is on screen, the one reading of "in view" a strip has: its days do
    /// not belong to a page, so there is no week for today to be inside of.
    private(set) var todayInView = false

    init(zoom: CalendarZoom) {
        self.zoom = zoom
        publish()
    }

    /// Republishes the derived facts, and only those that changed: an observable property notifies
    /// on every write, equal or not.
    private func publish() {
        if strip.anchorWeek != anchorWeek { anchorWeek = strip.anchorWeek }
        let left = strip.weeks * CGFloat(daysInWeek)
        // A column is on screen from the one the left edge is in up to, not including, the first
        // whose start lies past the right edge.
        let visible = Int(left.rounded(.down))..<Int((left + zoom.visibleDays).rounded(.up))
        let inView = todayColumn.map(visible.contains) ?? false
        if inView != todayInView { todayInView = inView }
    }
}
