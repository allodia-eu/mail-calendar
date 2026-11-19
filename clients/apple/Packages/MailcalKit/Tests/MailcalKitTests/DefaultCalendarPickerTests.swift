// Which calendars Settings → Calendar offers as the default (see DefaultCalendarPicker.swift).
// Which row is the default, and how calendars group under accounts, are the core's rules and are
// tested there.

import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct DefaultCalendarPickerTests {

    private func row(_ account: String, _ id: String, canWrite: Bool = true) -> CalendarRow {
        let swatch = Swatch(background: "#16598d", text: "#ffffff", border: "#16598d")
        return CalendarRow(
            account: account,
            accountAddress: "\(account)@test.local",
            id: id,
            name: id,
            color: CalendarColor(hex: "#16598d", light: swatch, dark: swatch),
            visible: true,
            canWrite: canWrite,
            canInvite: false,
            isDefault: false
        )
    }

    private func group(_ account: String, _ rows: [CalendarRow]) -> CalendarGroup {
        CalendarGroup(account: account, address: "\(account)@test.local", expanded: true, calendars: rows)
    }

    @Test func aReadOnlyCalendarIsNotOffered() {
        let groups = defaultCalendarGroups([group("a", [row("a", "work"), row("a", "holidays", canWrite: false)])])
        #expect(groups.map { $0.rows.map(\.id) } == [["work"]])
    }

    @Test func oneAccountNeedsNoHeader() {
        let groups = defaultCalendarGroups([group("a", [row("a", "work"), row("a", "home")])])
        #expect(groups.count == 1)
        #expect(groups[0].header == nil)
    }

    @Test func twoAccountsAreTwoGroupsNamedByAddress() {
        let groups = defaultCalendarGroups([
            group("a", [row("a", "work"), row("a", "home")]), group("b", [row("b", "work")]),
        ])
        #expect(groups.map(\.header) == ["a@test.local", "b@test.local"])
        #expect(groups.map { $0.rows.map(\.id) } == [["work", "home"], ["work"]])
    }

    @Test func nothingWritableIsNoGroups() {
        #expect(defaultCalendarGroups([group("a", [row("a", "holidays", canWrite: false)])]).isEmpty)
    }

    @Test func anAccountWithOnlyReadOnlyCalendarsIsNoGroup() {
        let groups = defaultCalendarGroups([
            group("a", [row("a", "work")]), group("b", [row("b", "holidays", canWrite: false)]),
        ])
        #expect(groups.count == 1)
        #expect(groups[0].header == nil)
    }
}
