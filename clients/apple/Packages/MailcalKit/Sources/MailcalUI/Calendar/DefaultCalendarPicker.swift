// Settings → Calendar → Default calendar (docs/settings.md, Calendar).
//
// The calendars offered, how they group under accounts and which is selected are the core's
// (`new_event_calendars`): the selection is `CalendarRow.isDefault`, resolved against the calendars
// that exist and can be written, so this picker and the calendar a new event opens on cannot
// disagree.

import MailcalBindings
import SwiftUI

/// A calendar named by its account and id, which is what the picker selects by.
private struct CalendarKey: Hashable {
    let account: String
    let id: String
}

/// Which calendar a new event is filed on.
struct DefaultCalendarPicker: View {
    var model: MailboxModel

    var body: some View {
        let offered = model.newEventCalendars()
        if offered.groups.isEmpty {
            Text(L10n.settings_default_calendar_none())
                .foregroundStyle(.secondary)
        } else {
            Picker(
                L10n.settings_default_calendar_heading(),
                selection: Binding(
                    get: {
                        offered.groups.flatMap(\.calendars).first(where: \.isDefault).map {
                            CalendarKey(account: $0.account, id: $0.id)
                        }
                    },
                    set: { key in
                        if let key { model.setDefaultCalendar(account: key.account, calendar: key.id) }
                    }
                )
            ) {
                ForEach(offered.groups, id: \.account) { group in
                    if offered.named {
                        Section(group.address) { rows(group) }
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

    private func rows(_ group: CalendarGroup) -> some View {
        ForEach(group.calendars, id: \.id) { calendar in
            Text(calendar.name)
                .tag(Optional(CalendarKey(account: calendar.account, id: calendar.id)))
        }
    }
}
