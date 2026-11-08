// The frame trace's summary: what counts as a dropped frame, and which gaps it reports.

import Testing

@testable import MailcalUI

@Suite struct CalendarFrameTraceTests {
    private let sixty = 1.0 / 60

    @Test func aFrameOnTimeIsNotDropped() {
        let stats = CalendarFrameStats(gaps: Array(repeating: sixty, count: 100), interval: sixty)
        #expect(stats?.dropped == 0)
        #expect(stats?.frames == 100)
    }

    @Test func aGapPastHalfAFrameLateIsADroppedFrame() {
        // A little jitter is the display link's, not a missed deadline; two intervals is one missed.
        var gaps = Array(repeating: sixty * 1.2, count: 98)
        gaps += [sixty * 2, sixty * 5]
        let stats = CalendarFrameStats(gaps: gaps, interval: sixty)
        #expect(stats?.dropped == 2)
        #expect(stats?.worst == sixty * 5)
        #expect(stats?.p50 == sixty * 1.2)
    }

    @Test func nothingToSummariseIsNoLine() {
        #expect(CalendarFrameStats(gaps: [], interval: sixty) == nil)
    }
}
