// The grid's pages are pulled once per calendar version, not once per frame.
//
// SwiftUI evaluates the grid's body on every frame of a scroll, a landing and a pinch, and each page
// is a layout in the core and a copy across the FFI. Pulled there, it is most of the frame.

import MailcalBindings
import Testing

@testable import MailcalUI

private func page(_ first: String) -> CalendarPage {
    CalendarPage(
        days: [GridDay(date: first)], timed: [], allDay: [], allDayLanes: 0,
        timezone: "Europe/Amsterdam", calendars: [], isMaterialized: true
    )
}

/// Counts the pulls that reached the core.
private final class Core {
    var pulls = 0
    func pull(_ from: String) -> CalendarPage? {
        pulls += 1
        return page(from)
    }
}

@Suite struct CalendarPageCacheTests {

    private func ask(
        _ cache: CalendarCache<CalendarPageKey, CalendarPage>, _ core: Core, _ from: String,
        version: Int = 1, zone: String = "Europe/Amsterdam"
    ) -> CalendarPage? {
        cache.value(CalendarPageKey(from: from, columns: 7), version: version, zone: zone) {
            core.pull(from)
        }
    }

    @Test func aWeekAskedForOnEveryFrameIsPulledOnce() {
        let cache = CalendarCache<CalendarPageKey, CalendarPage>()
        let core = Core()
        for _ in 0..<120 {
            #expect(ask(cache, core, "2026-11-02")?.days.first?.date == "2026-11-02")
            #expect(ask(cache, core, "2026-11-09")?.days.first?.date == "2026-11-09")
        }
        #expect(core.pulls == 2)
    }

    @Test func aNewCalendarVersionPullsAgain() {
        // The core's calendar signal is the only word that a page went stale: a sync landed, or a
        // calendar was hidden or recoloured.
        let cache = CalendarCache<CalendarPageKey, CalendarPage>()
        let core = Core()
        _ = ask(cache, core, "2026-11-02", version: 1)
        _ = ask(cache, core, "2026-11-02", version: 2)
        _ = ask(cache, core, "2026-11-02", version: 2)
        #expect(core.pulls == 2)
    }

    @Test func aNewDisplayZonePullsAgain() {
        let cache = CalendarCache<CalendarPageKey, CalendarPage>()
        let core = Core()
        _ = ask(cache, core, "2026-11-02", zone: "Europe/Amsterdam")
        _ = ask(cache, core, "2026-11-02", zone: "Asia/Tokyo")
        #expect(core.pulls == 2)
    }

    @Test func aPageTheCoreCouldNotGiveIsAskedForAgain() {
        // No core yet is not an answer to hold: the next frame must ask again.
        let cache = CalendarCache<CalendarPageKey, CalendarPage>()
        var pulls = 0
        for _ in 0..<2 {
            _ = cache.value(CalendarPageKey(from: "2026-11-02", columns: 7), version: 1, zone: "UTC") {
                pulls += 1
                return nil
            }
        }
        #expect(pulls == 2)
    }
}
