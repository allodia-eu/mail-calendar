// Dragging on the grid: what a gesture decided, and where a drop would leave the event.
//
// The twin of Android's `CalendarDrag.kt`, deliberately down to the names, this is a cross-platform
// contract (docs/calendar.md §13), and two clients that agree in prose but not in arithmetic have
// not kept it. Nothing here touches SwiftUI beyond `CGPoint`, so every decision is a pure function a
// test can drive without a two-finger gesture on a device.
//
// The rule it exists to hold: **a drag is a delta, not a destination.** What crosses the FFI is how
// far the hand moved, in whole days and minutes, never the clock the block was dropped under. A
// meeting in Amsterdam read on a Mac set to New York is drawn six hours earlier, and the delta is
// the same number in either zone, the reasoning in full is on `mailcal_account::apply_event_drag`.

import CoreGraphics
import Foundation
import MailcalBindings

/// The grid a drag snaps to, in minutes, the same quarter hour Android snaps to, and for the same
/// reason: it is what a diary is written in, and the delta is what gets snapped, so an event that
/// genuinely starts at 10:07 keeps its seven minutes when it is moved a day sideways.
let dragSnapMinutes = 15

/// How long a slot a press (or click) that never moved creates.
let defaultCreateMinutes = 60

/// Minutes in a day column. The grid is wall-clock, so this never changes.
let dayMinutes = 24 * 60

/// Minutes in an hour, the coarser grid a press fills.
let hourMinutes = 60

/// The hour band a press fills, the one the pointer is **inside**.
///
/// A press is "an event here", not a time to the minute, so it fills the hour it landed in rather
/// than the quarter-hour it happened to touch. The band **contains** the point, which is the property
/// that matters: rounding to the *nearest* boundary sends a press at 17:50 to 18:00–19:00, drawing
/// the whole block below the finger that asked for it. Takes the unrounded minute for the same
/// reason, 16:53 snaps forward to 17:00, and pinning from that lands in the next band along.
func hourBand(atMinute rawMinute: Int) -> Int {
    (rawMinute / hourMinutes * hourMinutes).clamped(to: 0...(dayMinutes - defaultCreateMinutes))
}

/// What a gesture on the grid turned out to be.
enum CalendarDragKind {
    /// The whole block moves: both edges by the same delta, so the duration is preserved exactly.
    case move
    /// The top edge moves; the end stays.
    case resizeStart
    /// The bottom edge moves; the start stays.
    case resizeEnd
    /// Empty grid: a new slot is being drawn out.
    case create
}

/// The event a move or resize is reshaping, captured when the gesture began.
///
/// Captured rather than looked up per frame: the page underneath can be re-pulled mid-gesture (a
/// sync lands and bumps `calendarVersion`), and a drag that re-resolved its subject would jump to a
/// different block, or to none.
struct CalendarDragSubject: Equatable {
    let account: String
    let event: String
    /// The occurrence's own start, as the core minted it, **empty when the event does not recur**.
    /// Non-empty is the signal to ask "this event, or all of them?" before writing anything.
    /// Opaque: it goes back across the FFI verbatim, never parsed here.
    let occurrenceStart: String
    let day: Int
    let startMinutes: Int
    let endMinutes: Int
}

/// Where a drag would leave a block, in the core's own geometry.
struct CalendarDragPreview: Equatable {
    let day: Int
    let startMinutes: Int
    let endMinutes: Int

    var minutes: Int { endMinutes - startMinutes }
}

/// A drag in flight: where it began, and where the pointer is now, both in the core's own currency,
/// so the deltas below are exactly what `Intent.moveEvent` wants.
struct CalendarDragState: Equatable {
    let kind: CalendarDragKind
    /// `nil` for a `.create`, which has no event yet.
    let subject: CalendarDragSubject?
    let anchorDay: Int
    let anchorMinute: Int
    var day: Int
    var minute: Int
    /// Where the pointer actually is, **unsnapped**, the picture's currency, never the write's.
    ///
    /// `minute` steps a quarter-hour at a time because that is what gets written; a block that moved
    /// only when it stepped would jump a dozen pixels at a zoomed-out horizon. So the renderer follows
    /// this (`livePreview()`), and the readout and `moveArgs` keep following `minute`.
    var rawMinute: Int
    /// Where the pointer went **down**, unrounded, what the hour band is measured from. `anchorMinute`
    /// is snapped, and 16:53 snaps forward to 17:00, which is the band below the one under the finger.
    let rawAnchorMinute: Int

    init(
        kind: CalendarDragKind,
        subject: CalendarDragSubject?,
        anchorDay: Int,
        anchorMinute: Int,
        day: Int,
        minute: Int,
        rawMinute: Int? = nil,
        rawAnchorMinute: Int? = nil
    ) {
        self.kind = kind
        self.subject = subject
        self.anchorDay = anchorDay
        self.anchorMinute = anchorMinute
        self.day = day
        self.minute = minute
        self.rawMinute = rawMinute ?? minute
        self.rawAnchorMinute = rawAnchorMinute ?? anchorMinute
    }

    var dayDelta: Int { day - anchorDay }
    var minuteDelta: Int { minute - anchorMinute }

    /// The unsnapped distance the pointer has travelled, `minuteDelta`'s picture-side twin.
    var rawMinuteDelta: Int { rawMinute - anchorMinute }

    /// The pointer moved: re-aim at where it is now, snapped and unsnapped alike.
    func moved(toDay day: Int, minute: Int, rawMinute: Int) -> CalendarDragState {
        var next = self
        next.day = day
        next.minute = minute
        next.rawMinute = rawMinute
        return next
    }

    /// Where the drag would leave things, what the preview draws, and what a create's editor opens
    /// on.
    ///
    /// A **create** stays in the column it began in: widening a slot across days is not a thing an
    /// event can be, so a sideways wobble while drawing one out must not file it on Wednesday. A
    /// **move** carries its column with the pointer.
    func preview() -> CalendarDragPreview { previewUsing(delta: minuteDelta) }

    /// Where the block is **drawn** while the pointer is still down.
    ///
    /// The same geometry as `preview()`, carried by the unsnapped delta, so the block glides with the
    /// hand rather than stepping a quarter-hour at a time. The two differ only mid-gesture, never by a
    /// whole snap step, and only on the edge in the hand, the anchored edge is drawn exactly where
    /// the write will put it. It is why the live readout exists: the pill says the **snapped** time,
    /// so nothing on screen claims a minute the drop will not honour.
    func livePreview() -> CalendarDragPreview { previewUsing(delta: rawMinuteDelta) }

    private func previewUsing(delta: Int) -> CalendarDragPreview {
        switch kind {
        case .create:
            // The slot is the **union** of the hour the press landed in and where the pointer is now.
            // Inside the band it is the band; below it the top stays on that hour and the bottom
            // follows; above it the bottom stays on the following hour and the top follows.
            //
            // A union rather than an anchor and a span, because a union is *continuous*: there is no
            // threshold at which the slot changes shape, so nothing can jump at one. An anchored span
            // has to choose an anchor, and choosing the press point makes the block leap off the hour
            // it was showing the moment the pointer moves. The cost, stated rather than discovered: a
            // drawn slot is never shorter than an hour, and shorter is the editor's job.
            let band = hourBand(atMinute: rawAnchorMinute)
            let pointer = anchorMinute + delta
            return CalendarDragPreview(
                day: anchorDay,
                startMinutes: min(band, pointer),
                endMinutes: max(band + defaultCreateMinutes, pointer)
            )
        case .move:
            guard let subject else { return CalendarDragPreview(day: day, startMinutes: 0, endMinutes: 0) }
            return CalendarDragPreview(
                day: subject.day + dayDelta,
                startMinutes: subject.startMinutes + delta,
                endMinutes: subject.endMinutes + delta
            )
        case .resizeStart:
            guard let subject else { return CalendarDragPreview(day: day, startMinutes: 0, endMinutes: 0) }
            return CalendarDragPreview(
                day: subject.day,
                startMinutes: subject.startMinutes + delta,
                endMinutes: subject.endMinutes
            )
        case .resizeEnd:
            guard let subject else { return CalendarDragPreview(day: day, startMinutes: 0, endMinutes: 0) }
            return CalendarDragPreview(
                day: subject.day,
                startMinutes: subject.startMinutes,
                endMinutes: subject.endMinutes + delta
            )
        }
    }

    /// Whether this drag changed anything, a press that went nowhere on an existing event writes
    /// nothing, rather than spending a network round-trip and a revision to change zero minutes.
    var movesAnything: Bool {
        switch kind {
        case .create: return true
        case .move: return dayDelta != 0 || minuteDelta != 0
        case .resizeStart, .resizeEnd: return minuteDelta != 0
        }
    }

    /// Whether a settled drag has to ask which occurrences it applies to before it writes.
    var asksAboutTheSeries: Bool { !(subject?.occurrenceStart.isEmpty ?? true) }

    /// Clamps the drag so its preview stays inside the grid it is drawn on.
    ///
    /// **What you see is what you get.** A move is clamped to the day's own midnight-to-midnight
    /// span, so an event dragged to the top of the screen stops at 00:00 rather than silently
    /// landing on the previous day, to change the day you drag *sideways*, which is what every
    /// calendar does and the one thing the preview can actually show. A resize is clamped so the
    /// edge being dragged cannot pass its opposite, matching the core's own floor.
    func clamped(toColumns columns: Int) -> CalendarDragState {
        var next = self
        next.day = day.clamped(to: 0...max(columns - 1, 0))
        // One pair of bounds, applied to the snapped minute and the raw one behind it. The picture may
        // be smoother than the write; it may not show a block off the end of the column, because that
        // is a write that cannot happen.
        let bounds: ClosedRange<Int>
        switch kind {
        case .create:
            bounds = 0...dayMinutes
        case .move:
            guard let subject else { return next }
            let lo = anchorMinute - subject.startMinutes
            let hi = anchorMinute + (dayMinutes - subject.endMinutes)
            bounds = min(lo, hi)...max(lo, hi)
        case .resizeStart:
            guard let subject else { return next }
            let hi = anchorMinute + (subject.endMinutes - dragSnapMinutes - subject.startMinutes)
            bounds = (anchorMinute - subject.startMinutes)...hi
        case .resizeEnd:
            guard let subject else { return next }
            let lo = anchorMinute - (subject.endMinutes - subject.startMinutes - dragSnapMinutes)
            bounds = lo...(anchorMinute + (dayMinutes - subject.endMinutes))
        }
        next.minute = minute.clamped(to: bounds)
        next.rawMinute = rawMinute.clamped(to: bounds)
        return next
    }

    /// The move a settled drag asks for, or `nil` if it drew out a new slot instead.
    ///
    /// `thisOccurrenceOnly` is the user's answer to "this event, or all of them?", asked only when
    /// the subject carries an occurrence token, and never guessed. Passing `false` for a one-off is
    /// correct and costs nothing: its token is empty, so there is no occurrence to name either way.
    func moveArgs(thisOccurrenceOnly: Bool) -> CalendarMoveArgs? {
        guard let subject else { return nil }
        let edge: EventEdge
        switch kind {
        case .resizeStart: edge = .start
        case .resizeEnd: edge = .end
        default: edge = .whole
        }
        let occurrence = thisOccurrenceOnly && !subject.occurrenceStart.isEmpty
            ? subject.occurrenceStart : nil
        return CalendarMoveArgs(
            account: subject.account,
            key: subject.event,
            edge: edge,
            days: Int32(dayDelta),
            minutes: Int32(minuteDelta),
            occurrence: occurrence
        )
    }
}

/// The arguments a settled drag dispatches (`Intent.moveEvent`).
struct CalendarMoveArgs: Equatable {
    let account: String
    let key: String
    let edge: EventEdge
    let days: Int32
    let minutes: Int32
    /// `nil` moves the whole series; a token names one occurrence.
    let occurrence: String?
}

// MARK: - Turning points into the core's geometry

/// The day column a **content**-space x falls in, past the hour ruler, before the day scroll.
func dragColumn(atX x: CGFloat, dayWidth: CGFloat) -> Int {
    dayWidth <= 0 ? 0 : Int((x / dayWidth).rounded(.down))
}

/// The wall-clock minute a **content**-space y falls on, snapped to the drag grid.
func dragMinute(atY y: CGFloat, hourHeight: CGFloat) -> Int {
    let raw = dragRawMinute(atY: y, hourHeight: hourHeight)
    return Int((CGFloat(raw) / CGFloat(dragSnapMinutes)).rounded()) * dragSnapMinutes
}

/// The wall-clock minute a **content**-space y falls on, to the minute.
///
/// What the block is drawn from while the pointer is down. Never what is written, `dragMinute` is.
func dragRawMinute(atY y: CGFloat, hourHeight: CGFloat) -> Int {
    guard hourHeight > 0 else { return 0 }
    return Int((y / hourHeight * 60).rounded())
}

/// How close to a block's edge a press must land for it to be a resize rather than a move.
///
/// Only applied once the block is at least three of these tall: below that the two zones would meet
/// and every press on a short event would be a resize of something whose middle you cannot reach.
let dragResizeEdge: CGFloat = 8

func dragResizeZoneApplies(blockHeight: CGFloat, edge: CGFloat = dragResizeEdge) -> Bool {
    blockHeight >= edge * 3
}

/// What a press at `point` (in the grid's **content** space) means, or `nil` if it means nothing.
///
/// The order is the order a hand expects: a block that is **the user's own** claims the press, its
/// edges claim it as a resize, and bare grid claims it as a create. A block that is not the user's
/// own claims nothing and falls through to a create, deliberate: a meeting somebody else called
/// cannot be re-timed here (docs/calendar.md §13), and doing nothing at all reads as a missed
/// gesture.
///
/// Hit-tested against the same rectangle `CalendarTimedBlock` draws, so a pointer and the pixels
/// agree.
func calendarDrag(
    at point: CGPoint,
    segments: [TimedSegment],
    dayCount: Int,
    dayWidth: CGFloat,
    hourHeight: CGFloat,
    canCreate: Bool
) -> CalendarDragState? {
    let day = dragColumn(atX: point.x, dayWidth: dayWidth)
    guard day >= 0, day < dayCount else { return nil }
    let minute = dragMinute(atY: point.y, hourHeight: hourHeight)
    // The raw press rides along: the hour band a create fills is the one the *pointer* is in, which
    // the snapped minute can no longer answer once it has rounded across a boundary.
    let rawMinute = dragRawMinute(atY: point.y, hourHeight: hourHeight)

    for segment in segments {
        // A segment clipped by midnight is not the event, its visible top or bottom is an artefact
        // of the column it is drawn in, so every gesture on it would mean something other than what
        // it looks like. Left undraggable, and said so in docs/calendar.md's Known gaps.
        guard segment.canMove, !segment.continuesBefore, !segment.continuesAfter else { continue }
        let columnWidth = dayWidth / CGFloat(segment.columns)
        let left = dayWidth * CGFloat(segment.day) + columnWidth * CGFloat(segment.column)
        let top = hourHeight * CGFloat(segment.startMinutes) / 60
        let bottom = hourHeight * CGFloat(segment.endMinutes) / 60
        guard point.x >= left, point.x < left + columnWidth, point.y >= top, point.y < bottom else {
            continue
        }
        let subject = CalendarDragSubject(
            account: segment.account,
            event: segment.event,
            occurrenceStart: segment.occurrenceStart,
            day: Int(segment.day),
            startMinutes: Int(segment.startMinutes),
            endMinutes: Int(segment.endMinutes)
        )
        var kind = CalendarDragKind.move
        if dragResizeZoneApplies(blockHeight: bottom - top) {
            if point.y - top <= dragResizeEdge {
                kind = .resizeStart
            } else if bottom - point.y <= dragResizeEdge {
                kind = .resizeEnd
            }
        }
        // A resize anchors on the edge it grabbed, not on the pointer: otherwise the first frame
        // jumps the edge to wherever inside the zone the pointer happened to land.
        let anchor: Int
        switch kind {
        case .resizeStart: anchor = subject.startMinutes
        case .resizeEnd: anchor = subject.endMinutes
        default: anchor = minute
        }
        return CalendarDragState(
            kind: kind, subject: subject,
            anchorDay: day, anchorMinute: anchor, day: day, minute: anchor
        )
    }

    guard canCreate else { return nil }
    return CalendarDragState(
        kind: .create, subject: nil,
        anchorDay: day, anchorMinute: minute, day: day, minute: minute,
        rawMinute: rawMinute, rawAnchorMinute: rawMinute
    )
}
