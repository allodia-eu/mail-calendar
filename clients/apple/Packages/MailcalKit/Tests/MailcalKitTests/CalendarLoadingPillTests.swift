// A period that is loading keeps its grid, with a spinner over it (`docs/calendar.md`, "Showing a
// period that is loading"). Hosted in a window of its own (`HostedScreen`), so none of it needs the
// app launched.

#if os(iOS) && DEBUG
import MailcalBindings
import SwiftUI
import Testing

@testable import MailcalUI

@Suite @MainActor struct CalendarLoadingPillTests {
    private let size = CGSize(width: 393, height: 700)

    /// A month whose 42 cells start on Monday 2026-10-26, `materialized` or not.
    private func month(materialized: Bool) -> HostedScreen {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Europe/Amsterdam") ?? .gmt
        let first = calendar.date(from: DateComponents(year: 2026, month: 10, day: 26)) ?? .now
        let cells = (0..<42).map { offset in
            let day = calendar.date(byAdding: .day, value: offset, to: first) ?? first
            let parts = calendar.dateComponents([.year, .month, .day], from: day)
            let date = String(
                format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0
            )
            return MonthCell(date: date, inMonth: parts.month == 11, chips: [])
        }
        let page = MonthPage(
            cells: cells, timezone: "Europe/Amsterdam", calendars: [],
            isMaterialized: materialized
        )
        return HostedScreen(
            CalendarMonthView(
                page: page, today: first, calendar: calendar, weekStartsMonday: true,
                use24Hour: true, onOpen: { _ in }
            ),
            size: size
        )
    }

    private func pills(_ screen: HostedScreen) -> [AgentNode] {
        screen.elements(role: "AXStaticText") { $0 == L10n.calendar_loading_range() }
    }

    /// Where the first cell's day number is drawn.
    private func firstDay(_ screen: HostedScreen) -> CGRect? {
        screen.elements(role: "AXStaticText") { $0 == "26" }.first?.bounds
    }

    @Test func aMonthStillLoadingKeepsItsCellsUnderThePill() throws {
        let loading = month(materialized: false)
        let pill = try #require(pills(loading).first, "no loading pill in:\n\(loading.dump())")
        let bounds = try #require(pill.bounds)
        // Centred over the cells, a few points under the weekday headings.
        let headings = try #require(
            loading.elements(role: "AXStaticText") { $0 == "Mon" }.first?.bounds
        )
        let gap = bounds.minY - headings.maxY
        #expect(gap > 0 && gap < 20, "the pill is at \(bounds), under headings at \(headings)")
        #expect(abs(bounds.midX - size.width / 2) < 2, "the pill is at \(bounds), not centred")
        // The cells are drawn, and where a materialized month draws them: the pill is laid over
        // the grid, never in a row of its own that pushes it down.
        let day = try #require(firstDay(loading), "no day cells in:\n\(loading.dump())")
        let settled = try #require(firstDay(month(materialized: true)))
        #expect(day == settled, "the first day moved from \(settled) to \(day) while loading")
    }

    @Test func aMaterializedMonthHasNoPill() {
        let screen = month(materialized: true)
        #expect(pills(screen).isEmpty, "a materialized month still shows:\n\(screen.dump())")
    }
}
#endif
