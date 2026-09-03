// The calendar grid's pointer handling: create, move and resize a block by drag, and pan the strip.
// Split out of CalendarGridView.swift to keep it under 500 lines.

import MailcalBindings
import SwiftUI

extension CalendarGridView {
    /// Whether `segment`, in week `week`, is the one currently in the pointer's hand.
    func isHeld(_ segment: TimedSegment, in week: Int) -> Bool {
        guard let held = drag?.subject, dragWeek == week else { return false }
        return segment.account == held.account && segment.event == held.event
            && Int(segment.day) == held.day
    }

    // MARK: - The pointer

    /// Everything the grid does with a pointer, arbitrated in **one** composed gesture.
    ///
    /// The two platforms genuinely differ here, and the difference is the input, not a preference:
    ///
    /// - **macOS.** A click-drag creates and moves; there is no drag-to-pan, because a desktop
    ///   *scrolls* a calendar, and `CalendarScrollGesture` reads the wheel and the trackpad on both
    ///   axes. This is what Apple's own Calendar and Google Calendar do on a Mac.
    /// - **iOS/iPadOS.** A plain drag is the *only* way a finger can pan, so it stays a pan, and a
    ///   **long press** is what takes hold of an event. Composed with `.sequenced`, so this is one
    ///   recognizer with a defined precedence rather than two reading the same finger: §6's
    ///   four-handlers-one-finger arrangement is exactly what must not be rebuilt here.
    var gridGesture: some Gesture {
        #if os(macOS)
            return dragGesture(minimumDistance: 2)
        #else
            return LongPressGesture(minimumDuration: 0.4, maximumDistance: 12)
                .sequenced(before: DragGesture(minimumDistance: 0))
                .onChanged { phase in
                    // `.first` is the press still being held; only `.second` carries a location.
                    guard case let .second(_, value?) = phase else { return }
                    if drag == nil { beginDrag(at: value.startLocation) }
                    updateDrag(to: value.location)
                }
                .onEnded { _ in endDrag() }
                // Until the press has been held, a moving finger is a pan, the same finger, one
                // recognizer, and the drag simply never recognises.
                .exclusively(before: panGesture)
        #endif
    }

    /// The macOS drag: create on empty grid, move or resize on the user's own block.
    private func dragGesture(minimumDistance: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: minimumDistance)
            .onChanged { value in
                if drag == nil { beginDrag(at: value.startLocation) }
                updateDrag(to: value.location)
            }
            .onEnded { _ in endDrag() }
    }

    /// One-finger pan, both axes, iOS/iPadOS only. A pinch is two fingers and is handled elsewhere;
    /// this never sees it.
    ///
    /// Each frame reports **its own delta** rather than the translation since the finger went down.
    /// The strip is the one owner of the position, and handing it a cumulative translation would
    /// mean two places holding "where the grid is", which is two places that can disagree the moment
    /// anything else (a pinch, a landing, a jump home) moves it.
    private var panGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                let last = panned ?? .zero
                onScroll(
                    value.translation.width - last.width, value.translation.height - last.height
                )
                panned = value.translation
            }
            .onEnded { value in
                panned = nil
                // `predictedEndTranslation` is UIKit's own decay estimate, which is why a flick
                // feels native rather than invented, and why it carries about a screenful on a
                // phone. What is left of it is handed on as momentum, and the day axis lands from
                // wherever that coast ends.
                onScrollEnded(
                    value.predictedEndTranslation.width - value.translation.width,
                    value.predictedEndTranslation.height - value.translation.height
                )
            }
    }

    // MARK: - Dragging a block

    /// A press stays inside the week it landed in, and every coordinate below is that week's own.
    ///
    /// The strip is continuous, but an event's geometry is not: the core lays a page out per week,
    /// and a delta that crossed a seam would be measured against a page that does not hold the day
    /// it started on. So a drag is resolved against one week's page, and clamps to its seven
    /// columns, which is what it did when a week *was* the whole grid.
    private func week(at point: CGPoint) -> (index: Int, week: CalendarStripWeek, content: CGPoint)? {
        let x = point.x - calendarGutter
        // The gesture is attached to the row that holds the ruler as well as the columns, so a press
        // on the gutter arrives with a negative x. The strip has no left edge to stop at, so that
        // resolves to the *previous* week's last day rather than to nothing: a press on the hour
        // ruler would draw a new event on a Sunday nobody is looking at.
        guard x >= 0 else { return nil }
        let index = strip.location(atX: x, dayWidth: dayWidth).week
        guard let found = weekAt(index) else { return nil }
        return (
            index,
            found,
            CGPoint(
                x: x - strip.origin(ofWeek: index, dayWidth: dayWidth),
                y: point.y + hourOffset
            )
        )
    }

    private func beginDrag(at point: CGPoint) {
        guard let target = week(at: point) else { return }
        drag = calendarDrag(
            at: target.content,
            segments: target.week.page.timed,
            dayCount: target.week.days.count,
            dayWidth: dayWidth,
            hourHeight: hourHeight,
            canCreate: canCreateEvent
        )
        dragWeek = drag == nil ? nil : target.index
    }

    private func updateDrag(to point: CGPoint) {
        guard let live = drag, let index = dragWeek else { return }
        let content = CGPoint(
            x: point.x - calendarGutter - strip.origin(ofWeek: index, dayWidth: dayWidth),
            y: point.y + hourOffset
        )
        drag = live.moved(
            toDay: dragColumn(atX: content.x, dayWidth: dayWidth),
            minute: dragMinute(atY: content.y, hourHeight: hourHeight),
            rawMinute: dragRawMinute(atY: content.y, hourHeight: hourHeight)
        ).clamped(toColumns: daysInWeek)
    }

    private func endDrag() {
        guard let settled = drag, let index = dragWeek else { return }
        drag = nil
        dragWeek = nil
        // A press that went nowhere on an existing event is a press, not an edit.
        if settled.movesAnything { onDrop(index, settled) }
    }
}
