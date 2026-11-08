// The frame trace's scripted motion: the grid moves itself, one axis at a time, the same way on
// every run, so two builds can be compared without a hand on the trackpad.
//
//     MAILCAL_CALENDAR_TRACE=drive      # opens the calendar, then hours, days and zoom in turn
//
// Each burst is followed by a pause long enough for the trace to close it, so the log carries one
// line per axis. It moves the grid's position directly, so it measures what a frame costs to draw,
// not the gesture that would have asked for it; that half is measured by hand.

import SwiftUI

extension CalendarFrameTrace {
    /// Whether the trace was asked to move the grid itself.
    var drives: Bool { ProcessInfo.processInfo.environment["MAILCAL_CALENDAR_TRACE"] == "drive" }
}

/// Runs the scripted bursts against `position`, then puts the zoom back where it was.
@MainActor
func driveCalendarForMeasurement(_ position: CalendarGridPosition) async {
    // Let the first page land and the grid frame itself on today.
    guard (try? await Task.sleep(for: .seconds(3))) != nil else { return }
    let hours = position.hourOffset
    let weeks = position.strip.weeks
    let zoom = position.zoom
    await burst { phase in
        CalendarFrameTrace.shared.moved(.hours)
        position.hourOffset = max(hours + 300 * sin(phase * 2 * .pi), 0)
    }
    guard !Task.isCancelled else { return }
    await burst { phase in
        CalendarFrameTrace.shared.moved(.days)
        position.strip.weeks = weeks + 1.5 * sin(phase * 2 * .pi)
    }
    var last: CGFloat = 1
    guard !Task.isCancelled else { return }
    await burst { phase in
        CalendarFrameTrace.shared.moved(.zoom)
        // Out to two and a half times the hours and days, and back, one step per frame.
        let scale = 1 + 0.75 * (1 - cos(phase * 2 * .pi))
        _ = position.zoom.pinchVertical(scale / last)
        _ = position.zoom.pinchHorizontal(scale / last)
        last = scale
    }
    position.zoom = zoom
    position.strip.weeks = weeks
    position.hourOffset = hours
    logAppleLifecycle("calendar frames: scripted motion done")
}

/// Four seconds of motion, stepped as fast as the main actor allows, then a second's rest. Stops
/// when the task is cancelled (the calendar left the screen): a sleep that throws returns at once,
/// so carrying on would spin the main actor for what is left of the burst.
@MainActor
private func burst(_ step: (CGFloat) -> Void) async {
    let start = ContinuousClock.now
    let length = Duration.seconds(4)
    while !Task.isCancelled {
        let elapsed = ContinuousClock.now - start
        guard elapsed < length else { break }
        step(CGFloat(elapsed / length))
        guard (try? await Task.sleep(for: .milliseconds(4))) != nil else { return }
    }
    _ = try? await Task.sleep(for: .seconds(1))
}
