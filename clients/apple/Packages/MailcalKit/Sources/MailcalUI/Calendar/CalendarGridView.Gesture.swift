// The calendar grid's pointer handling: create, move and resize a block by drag, and pan the
// grid. Split out of CalendarGridView.swift to keep it under 500 lines.

import MailcalBindings
import SwiftUI

extension CalendarGridView {
    /// Whether `segment` is the one currently in the pointer's hand.
    func isHeld(_ segment: TimedSegment) -> Bool {
        guard let held = drag?.subject else { return false }
        return segment.account == held.account && segment.event == held.event
            && Int(segment.day) == held.day
    }

    // MARK: - The pointer

    /// Everything the grid does with a pointer, arbitrated in **one** composed gesture.
    ///
    /// The two platforms genuinely differ here, and the difference is the input, not a preference:
    ///
    /// - **macOS.** A click-drag creates and moves; there is no drag-to-pan any more, because a
    ///   desktop *scrolls* a calendar, `CalendarScrollGesture` already reads the wheel and the
    ///   trackpad on both axes (docs/calendar.md §11 note ⁷). This is what Apple's own Calendar and
    ///   Google Calendar do on a Mac, and it is why taking the pan away costs nothing.
    /// - **iOS/iPadOS.** A plain drag is the *only* way to pan, there is no wheel, so it stays a
    ///   pan, and a **long press** is what takes hold of an event. Composed with `.sequenced`, so
    ///   this is one recognizer with a defined precedence rather than two reading the same finger:
    ///   §6's four-handlers-one-finger arrangement is exactly what must not be rebuilt here.
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

    /// One-finger pan, both axes, iOS/iPadOS only now. A pinch is two fingers and is handled
    /// elsewhere; this never sees it.
    private var panGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                let start = dragStart ?? CGPoint(x: dayOffset, y: hourOffset)
                if dragStart == nil { dragStart = start }
                dayOffset = (start.x - value.translation.width).clamped(to: 0...maxDayOffset)
                hourOffset = (start.y - value.translation.height).clamped(to: 0...maxHourOffset)
            }
            .onEnded { value in
                dragStart = nil
                // A little momentum, so a flick keeps going the way a scroll view would. `predicted`
                // is UIKit's own decay estimate, which is why it feels native rather than invented.
                let predictedX = (dayOffset - (value.predictedEndTranslation.width - value.translation.width))
                let predictedY = (hourOffset - (value.predictedEndTranslation.height - value.translation.height))
                withAnimation(.easeOut(duration: 0.35)) {
                    dayOffset = predictedX.clamped(to: 0...maxDayOffset)
                    hourOffset = predictedY.clamped(to: 0...maxHourOffset)
                }
            }
    }

    /// A gesture's point, in the **grid content's** own space: the gesture is attached to the row
    /// that holds the hour ruler and the grid, so the ruler's width comes off the x, and both scroll
    /// offsets go back on, the content is drawn shifted by them.
    private func contentPoint(_ point: CGPoint) -> CGPoint {
        CGPoint(x: point.x - calendarGutter + dayOffset, y: point.y + hourOffset)
    }

    private func beginDrag(at point: CGPoint) {
        drag = calendarDrag(
            at: contentPoint(point),
            segments: page.timed,
            dayCount: days.count,
            dayWidth: dayWidth,
            hourHeight: hourHeight,
            canCreate: canCreateEvent
        )
    }

    private func updateDrag(to point: CGPoint) {
        guard let live = drag else { return }
        let content = contentPoint(point)
        drag = live.moved(
            toDay: dragColumn(atX: content.x, dayWidth: dayWidth),
            minute: dragMinute(atY: content.y, hourHeight: hourHeight),
            rawMinute: dragRawMinute(atY: content.y, hourHeight: hourHeight)
        ).clamped(toColumns: days.count)
    }

    private func endDrag() {
        guard let settled = drag else { return }
        drag = nil
        // A press that went nowhere on an existing event is a press, not an edit.
        if settled.movesAnything { onDrop(settled) }
    }
}
