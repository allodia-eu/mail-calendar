// Settings → Calendar → Default calendar (docs/settings.md category 3).
//
// The selection is the core's `CalendarRow.isDefault`, never a rule kept here: the core resolves
// the stored choice against the calendars that exist and can be written, so this picker and the
// calendar a new event opens on cannot disagree.

import MailcalBindings
import SwiftUI

/// One account's writable calendars, as the picker lists them.
struct DefaultCalendarGroup: Equatable {
    /// The account's address, or `nil` while every calendar is one account's: its rows then need
    /// nothing to tell them apart.
    let header: String?
    let rows: [CalendarRow]
}

/// The calendars a new event can be filed on, grouped by account in the core's order. A calendar
/// id is unique only within its account, so two accounts are two groups.
func defaultCalendarGroups(_ calendars: [CalendarRow]) -> [DefaultCalendarGroup] {
    var order: [String] = []
    var groups: [String: [CalendarRow]] = [:]
    for row in calendars where row.canWrite {
        if groups[row.account] == nil { order.append(row.account) }
        groups[row.account, default: []].append(row)
    }
    return order.map { account in
        let rows = groups[account] ?? []
        return DefaultCalendarGroup(
            header: order.count > 1 ? rows.first?.accountAddress ?? account : nil,
            rows: rows
        )
    }
}

/// A calendar named by its account and id, which is what the picker selects by.
private struct CalendarKey: Hashable {
    let account: String
    let id: String
}

/// Which calendar a new event is filed on.
struct DefaultCalendarPicker: View {
    var model: MailboxModel

    var body: some View {
        // The core signals the calendar surface when the default changes; reading the version
        // makes this view pull the list again.
        let _ = model.calendarVersion
        let groups = defaultCalendarGroups(model.calendars())
        if groups.isEmpty {
            Text(L10n.settings_default_calendar_none())
                .foregroundStyle(.secondary)
        } else {
            Picker(
                L10n.settings_default_calendar_heading(),
                selection: Binding(
                    get: {
                        groups.flatMap(\.rows).first(where: \.isDefault).map {
                            CalendarKey(account: $0.account, id: $0.id)
                        }
                    },
                    set: { key in
                        if let key { model.setDefaultCalendar(account: key.account, calendar: key.id) }
                    }
                )
            ) {
                ForEach(groups, id: \.rows.first?.account) { group in
                    if let header = group.header {
                        Section(header) { rows(group) }
                    } else {
                        rows(group)
                    }
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .fixedSize()
            .accessibilityIdentifier("settings-default-calendar")
        }
    }

    private func rows(_ group: DefaultCalendarGroup) -> some View {
        ForEach(group.rows, id: \.id) { calendar in
            Text(calendar.name)
                .tag(Optional(CalendarKey(account: calendar.account, id: calendar.id)))
        }
    }
}
