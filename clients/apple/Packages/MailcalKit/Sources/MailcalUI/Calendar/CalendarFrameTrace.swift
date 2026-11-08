// The time grid's frame gaps during motion, in numbers: Apple's half of docs/calendar.md §7.
//
// A display link times every frame while the grid is moving, and once the motion stops the gaps are
// summarised into one log line: how many frames, how many missed their deadline, and the gaps at the
// median, p90, p99 and worst. The gap between one frame and the next during the motion is what the
// eye sees; an average over idle frames hides it. The line also says what moved the grid (how many
// input events went to the days, the hours and the zoom) and how many weeks' blocks were built: a
// scroll that builds weeks other than the one sliding in is doing work it should not.
//
// Off unless `MAILCAL_CALENDAR_TRACE` is set at launch, in **any** build, because a release build is
// the only one worth judging the grid on. Switched off it costs one cached boolean per grid redraw.
// It logs counts and durations only, never what is on the grid (docs/logging.md).
//
//     scripts/dev/device.sh run MAILCAL_CALENDAR_TRACE=1      # then scroll, pinch, fling
//     scripts/dev/device.sh logs --grep 'calendar frames'

import Foundation
import QuartzCore
#if os(macOS)
    import AppKit
#endif

/// One burst of motion, summarised.
struct CalendarFrameStats: Equatable {
    let frames: Int
    /// Frames whose gap ran past half a frame beyond the display's interval: at least one missed.
    let dropped: Int
    let p50: Double
    let p90: Double
    let p99: Double
    let worst: Double

    /// `gaps` and `interval` in seconds; nil when there is nothing to summarise.
    init?(gaps: [Double], interval: Double) {
        guard !gaps.isEmpty, interval > 0 else { return nil }
        let sorted = gaps.sorted()
        func at(_ quantile: Double) -> Double {
            sorted[min(Int((Double(sorted.count - 1) * quantile).rounded()), sorted.count - 1)]
        }
        frames = gaps.count
        dropped = gaps.filter { $0 > interval * 1.5 }.count
        p50 = at(0.5)
        p90 = at(0.9)
        p99 = at(0.99)
        worst = sorted[sorted.count - 1]
    }

    /// The log line: milliseconds, and the refresh rate the gaps were judged against.
    func line(interval: Double) -> String {
        func ms(_ seconds: Double) -> String { String(format: "%.1f", seconds * 1000) }
        let percent = String(format: "%.1f", Double(dropped) * 100 / Double(frames))
        return "calendar frames: \(frames) in motion at \(Int((1 / interval).rounded())) Hz, "
            + "\(dropped) dropped (\(percent)%), gap p50 \(ms(p50)) p90 \(ms(p90)) "
            + "p99 \(ms(p99)) worst \(ms(worst)) ms"
    }
}

/// What an input did to the grid.
enum CalendarMotion: String, CaseIterable, CustomStringConvertible {
    case days, hours, zoom

    var description: String { rawValue }
}

@MainActor
final class CalendarFrameTrace: NSObject {
    static let shared = CalendarFrameTrace()

    let on = ProcessInfo.processInfo.environment["MAILCAL_CALENDAR_TRACE"].map {
        !["", "0", "false", "no", "off"].contains($0.lowercased())
    } ?? false

    private var link: AnyObject?
    private var lastRedraw: CFTimeInterval = 0
    private var lastFrame: CFTimeInterval?
    private var interval: CFTimeInterval = 1.0 / 60
    private var gaps: [Double] = []
    private var moves: [CalendarMotion: Int] = [:]
    private var weeksBuilt = 0

    /// An input moved the grid along `motion`.
    func moved(_ motion: CalendarMotion) {
        guard on else { return }
        moves[motion, default: 0] += 1
    }

    /// One week's blocks were built: on entering the screen, on a calendar change, and on every frame
    /// of a pinch, but never on a frame that only moves the grid.
    func weekBuilt() {
        if on { weeksBuilt += 1 }
    }

    /// The grid drew a frame: it is moving, so its frames are timed until it stops.
    func gridRedrawn() {
        guard on else { return }
        lastRedraw = CACurrentMediaTime()
        if link == nil { start() }
    }

    private func start() {
        #if os(iOS)
            let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
            link.add(to: .main, forMode: .common)
            self.link = link
        #else
            guard let link = NSScreen.main?.displayLink(target: self, selector: #selector(tick(_:)))
            else { return }
            link.add(to: .main, forMode: .common)
            self.link = link
        #endif
    }

    @objc private func tick(_ link: CADisplayLink) {
        let now = link.timestamp
        let frame = link.targetTimestamp - link.timestamp
        if frame > 0 { interval = frame }
        // Still moving if the grid drew within the last two frames; past that, the burst is over.
        guard now - lastRedraw < interval * 2 + 0.05 else {
            flush()
            link.invalidate()
            self.link = nil
            lastFrame = nil
            return
        }
        if let lastFrame { gaps.append(now - lastFrame) }
        lastFrame = now
    }

    private func flush() {
        defer {
            gaps.removeAll(keepingCapacity: true)
            moves.removeAll(keepingCapacity: true)
            weeksBuilt = 0
        }
        // A burst of a handful of frames is a tap or a nudge, not motion worth judging.
        guard gaps.count >= 10, let stats = CalendarFrameStats(gaps: gaps, interval: interval) else {
            return
        }
        let inputs = CalendarMotion.allCases.map { "\($0) \(moves[$0, default: 0])" }
            .joined(separator: ", ")
        logAppleLifecycle(
            stats.line(interval: interval) + "; inputs: \(inputs); weeks built \(weeksBuilt)"
        )
    }
}
