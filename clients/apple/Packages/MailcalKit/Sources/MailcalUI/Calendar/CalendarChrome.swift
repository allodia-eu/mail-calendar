// The calendar's header: the period title, the shape picker, "back to today", and the week arrows.
//
// The "back to today" affordance is the one Samsung gets right and most calendars get wrong: a
// calendar glyph with *today's date number inside it*, shown only when today is not on screen. It
// tells you where you'd land, and disappears once you're there, so it never sits in the bar as dead
// chrome.

import MailcalBindings
import SwiftUI

struct CalendarHeaderBar: View {
    let title: String
    let mode: CalendarMode
    let today: Date
    let todayVisible: Bool
    let calendar: Calendar
    let writeStatus: CalendarWriteStatus
    let canCreateEvent: Bool
    let onMode: (CalendarMode) -> Void
    let onBackToToday: () -> Void
    let onNewEvent: () -> Void
    let onManage: () -> Void
    let onRefresh: () -> Void
    let onPrevious: () -> Void
    let onNext: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Text(title).font(.headline)
            Spacer(minLength: 8)

            // The result of the user's last create/edit/delete: a spinner while it settles, a
            // check when saved, a tap-to-retry warning when it could not be confirmed. The retry
            // is a refresh, a full sync reconciles the local view (see CalendarWriteStatus).
            CalendarWriteBadge(status: writeStatus, onRetry: onRefresh)

            if !todayVisible {
                Button(action: onBackToToday) {
                    Text("\(calendar.component(.day, from: today))")
                        .font(.caption.weight(.semibold))
                        .frame(width: 22, height: 22)
                        .overlay(
                            RoundedRectangle(cornerRadius: 5).strokeBorder(lineWidth: 1.5)
                        )
                }
                .buttonStyle(.plain)
                .help(L10n.calendar_back_to_today())
                .accessibilityLabel(L10n.calendar_back_to_today())
            }

            Button(action: onPrevious) { Image(systemName: "chevron.left") }
                .buttonStyle(.borderless)
            Button(action: onNext) { Image(systemName: "chevron.right") }
                .buttonStyle(.borderless)

            Button(action: onNewEvent) {
                Label(L10n.action_new_event(), systemImage: "calendar.badge.plus")
            }
            .labelStyle(.iconOnly)
            .help(L10n.action_new_event())
            // Disabled, not hidden, when no calendar on the page can take a write, so the
            // header keeps its shape (see calendarSupportsNewEvent).
            .disabled(!canCreateEvent)

            Menu {
                Picker(
                    L10n.calendar_view_label(),
                    selection: Binding(get: { mode }, set: { onMode($0) })
                ) {
                    ForEach(CalendarMode.allCases) { entry in
                        Text(label(for: entry)).tag(entry)
                    }
                }
                .pickerStyle(.inline)
                Divider()
                Button(L10n.calendar_manage(), action: onManage)
                Button(L10n.action_refresh(), action: onRefresh)
            } label: {
                Label(L10n.calendar_view_label(), systemImage: "ellipsis.circle")
            }
            .labelStyle(.iconOnly)
            .menuStyle(.borderlessButton)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    private func label(for mode: CalendarMode) -> String {
        switch mode {
        case .day: return L10n.calendar_view_day()
        case .threeDay: return L10n.calendar_view_three_day()
        case .workWeek: return L10n.calendar_view_work_week()
        case .week: return L10n.calendar_view_week()
        case .month: return L10n.calendar_view_month()
        case .agenda: return L10n.calendar_view_agenda()
        }
    }
}
